//! Exact certified-region lowering into the canonical case decision DAG.
//!
//! A proof backend supplies disjoint rectangles or interval/congruence cells
//! over canonical generator-axis domain ordinals. This module validates their finite
//! support, retains opaque certificate identities for accounting, and lowers
//! the correlated union as one total decision partition. Uncovered support is
//! always eligibility-open; it is never inferred from neighboring regions.
//!
//! Program/query hash validation and certificate checking belong upstream.
//! Case classification certificates do not carry or imply mechanism
//! signatures.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::case_graph::{
    CaseDecisionDag, CaseGraphError, CaseOpenReason, CaseTerminal, CheckedCardinality,
    DecisionPartition, DecisionPartitionArc, DecisionPartitionTarget,
};

/// Default artifact-wide limit for expanding non-unit congruences into the
/// exact interval-union representation used by the case graph.
pub(super) const DEFAULT_MAX_CONGRUENCE_ORDINAL_INTERVALS: u128 = 1_000_000;

/// Deterministic resource policy for certified-region lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CertifiedRegionLoweringLimits {
    max_congruence_ordinal_intervals: u128,
}

impl CertifiedRegionLoweringLimits {
    pub(super) const fn new(max_congruence_ordinal_intervals: u128) -> Self {
        Self {
            max_congruence_ordinal_intervals,
        }
    }

    pub(super) const fn max_congruence_ordinal_intervals(self) -> u128 {
        self.max_congruence_ordinal_intervals
    }
}

impl Default for CertifiedRegionLoweringLimits {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CONGRUENCE_ORDINAL_INTERVALS)
    }
}

/// One nonempty half-open interval in source-domain ordinal space.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct CertifiedOrdinalInterval {
    start: u128,
    end_exclusive: u128,
}

impl CertifiedOrdinalInterval {
    pub(super) fn new(start: u128, end_exclusive: u128) -> Result<Self, CertifiedRegionShapeError> {
        if start >= end_exclusive {
            return Err(CertifiedRegionShapeError::InvalidInterval {
                start,
                end_exclusive,
            });
        }
        Ok(Self {
            start,
            end_exclusive,
        })
    }

    pub(super) const fn start(self) -> u128 {
        self.start
    }

    pub(super) const fn end_exclusive(self) -> u128 {
        self.end_exclusive
    }

    const fn len(self) -> u128 {
        self.end_exclusive - self.start
    }
}

/// One exact component of an axis set.
///
/// A congruence component selects the ordinals in `interval` whose canonical
/// ordinal has the given residue. The current public case graph represents
/// exact ordinal sets as interval unions, so non-unit congruences lower to
/// singleton ordinal intervals on this axis, not to singleton case paths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct CertifiedOrdinalCell {
    kind: CertifiedOrdinalCellKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum CertifiedOrdinalCellKind {
    Interval(CertifiedOrdinalInterval),
    IntervalCongruence {
        interval: CertifiedOrdinalInterval,
        modulus: u128,
        residue: u128,
    },
}

impl CertifiedOrdinalCell {
    pub(super) const fn interval(interval: CertifiedOrdinalInterval) -> Self {
        Self {
            kind: CertifiedOrdinalCellKind::Interval(interval),
        }
    }

    pub(super) fn interval_congruence(
        interval: CertifiedOrdinalInterval,
        modulus: u128,
        residue: u128,
    ) -> Result<Self, CertifiedRegionShapeError> {
        if modulus == 0 {
            return Err(CertifiedRegionShapeError::ZeroModulus);
        }
        if residue >= modulus {
            return Err(CertifiedRegionShapeError::ResidueOutOfRange { modulus, residue });
        }
        Ok(Self {
            kind: CertifiedOrdinalCellKind::IntervalCongruence {
                interval,
                modulus,
                residue,
            },
        })
    }

    const fn interval_bounds(self) -> CertifiedOrdinalInterval {
        match self.kind {
            CertifiedOrdinalCellKind::Interval(interval)
            | CertifiedOrdinalCellKind::IntervalCongruence { interval, .. } => interval,
        }
    }
}

/// A nonempty exact set on one declared source axis.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct CertifiedAxisSet {
    cells: Box<[CertifiedOrdinalCell]>,
}

impl CertifiedAxisSet {
    pub(super) fn new(
        cells: impl Into<Box<[CertifiedOrdinalCell]>>,
    ) -> Result<Self, CertifiedRegionShapeError> {
        let cells = cells.into();
        if cells.is_empty() {
            return Err(CertifiedRegionShapeError::EmptyAxisSet);
        }
        Ok(Self { cells })
    }

    pub(super) fn interval(interval: CertifiedOrdinalInterval) -> Self {
        Self {
            cells: vec![CertifiedOrdinalCell::interval(interval)].into_boxed_slice(),
        }
    }

    pub(super) fn interval_congruence(
        interval: CertifiedOrdinalInterval,
        modulus: u128,
        residue: u128,
    ) -> Result<Self, CertifiedRegionShapeError> {
        Self::new(
            vec![CertifiedOrdinalCell::interval_congruence(
                interval, modulus, residue,
            )?]
            .into_boxed_slice(),
        )
    }

    pub(super) fn cells(&self) -> &[CertifiedOrdinalCell] {
        &self.cells
    }
}

/// One proof-backed, correlated product cell.
///
/// The axes are a conjunction in canonical generator-axis order. Multiple regions form a union;
/// their per-axis supports are never merged into marginal unions.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct CertifiedCaseRegion<Certificate> {
    axes: Box<[CertifiedAxisSet]>,
    classification: CaseTerminal,
    certificate: Certificate,
}

impl<Certificate> CertifiedCaseRegion<Certificate> {
    pub(super) fn new(
        axes: impl Into<Box<[CertifiedAxisSet]>>,
        classification: CaseTerminal,
        certificate: Certificate,
    ) -> Self {
        Self {
            axes: axes.into(),
            classification,
            certificate,
        }
    }

    pub(super) fn rectangle(
        axes: impl IntoIterator<Item = CertifiedOrdinalInterval>,
        classification: CaseTerminal,
        certificate: Certificate,
    ) -> Self {
        Self::new(
            axes.into_iter()
                .map(CertifiedAxisSet::interval)
                .collect::<Vec<_>>(),
            classification,
            certificate,
        )
    }

    pub(super) fn axes(&self) -> &[CertifiedAxisSet] {
        &self.axes
    }

    pub(super) fn classification(&self) -> &CaseTerminal {
        &self.classification
    }

    pub(super) fn certificate(&self) -> &Certificate {
        &self.certificate
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CertifiedRegionShapeError {
    InvalidInterval { start: u128, end_exclusive: u128 },
    ZeroModulus,
    ResidueOutOfRange { modulus: u128, residue: u128 },
    EmptyAxisSet,
}

impl fmt::Display for CertifiedRegionShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInterval {
                start,
                end_exclusive,
            } => write!(
                formatter,
                "certified ordinal interval [{start}, {end_exclusive}) is empty or reversed"
            ),
            Self::ZeroModulus => write!(formatter, "certified congruence modulus must be positive"),
            Self::ResidueOutOfRange { modulus, residue } => write!(
                formatter,
                "certified congruence residue {residue} is outside modulus {modulus}"
            ),
            Self::EmptyAxisSet => write!(formatter, "certified axis set must be nonempty"),
        }
    }
}

impl Error for CertifiedRegionShapeError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct NormalizedAxisSet {
    intervals: Box<[CertifiedOrdinalInterval]>,
    cardinality: u128,
}

impl NormalizedAxisSet {
    fn contains(&self, ordinal: u128) -> bool {
        self.intervals
            .binary_search_by(|interval| {
                if ordinal < interval.start {
                    std::cmp::Ordering::Greater
                } else if ordinal >= interval.end_exclusive {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    fn intersects(&self, other: &Self) -> bool {
        let mut left = 0;
        let mut right = 0;
        while left < self.intervals.len() && right < other.intervals.len() {
            let lhs = self.intervals[left];
            let rhs = other.intervals[right];
            if lhs.start < rhs.end_exclusive && rhs.start < lhs.end_exclusive {
                return true;
            }
            if lhs.end_exclusive <= rhs.end_exclusive {
                left += 1;
            } else {
                right += 1;
            }
        }
        false
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct NormalizedRegion<Certificate> {
    axes: Box<[NormalizedAxisSet]>,
    classification: CaseTerminal,
    certificate: Certificate,
    cardinality: u128,
}

/// Exact multiplicities of every total case-graph terminal class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CertifiedCaseCounts {
    universe: u128,
    excluded: u128,
    eligibility_open: u128,
    admissible_nonmatch: u128,
    admissible_match: u128,
    admissible_open: u128,
}

impl CertifiedCaseCounts {
    pub(super) const fn universe(self) -> u128 {
        self.universe
    }

    pub(super) const fn excluded(self) -> u128 {
        self.excluded
    }

    pub(super) const fn eligibility_open(self) -> u128 {
        self.eligibility_open
    }

    pub(super) const fn admissible_nonmatch(self) -> u128 {
        self.admissible_nonmatch
    }

    pub(super) const fn admissible_match(self) -> u128 {
        self.admissible_match
    }

    pub(super) const fn admissible_open(self) -> u128 {
        self.admissible_open
    }

    pub(super) fn known_admissible(self) -> u128 {
        self.admissible_nonmatch
            .checked_add(self.admissible_match)
            .and_then(|count| count.checked_add(self.admissible_open))
            .expect("validated terminal counts cannot exceed the universe")
    }

    pub(super) fn admissible_configurations(self) -> CertifiedClosureCount {
        if self.eligibility_open == 0 {
            CertifiedClosureCount::Exact(self.known_admissible())
        } else {
            CertifiedClosureCount::LowerBound(self.known_admissible())
        }
    }

    pub(super) const fn matching_configurations(self) -> CertifiedClosureCount {
        if self.eligibility_open == 0 && self.admissible_open == 0 {
            CertifiedClosureCount::Exact(self.admissible_match)
        } else {
            CertifiedClosureCount::LowerBound(self.admissible_match)
        }
    }
}

/// Whether exact terminal multiplicities close the semantic aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CertifiedClosureCount {
    Exact(u128),
    LowerBound(u128),
}

/// Exact support attributed to one opaque certificate and classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CertifiedProofSupport<Certificate> {
    certificate: Certificate,
    classification: CaseTerminal,
    cardinality: u128,
}

impl<Certificate> CertifiedProofSupport<Certificate> {
    pub(super) fn certificate(&self) -> &Certificate {
        &self.certificate
    }

    pub(super) fn classification(&self) -> &CaseTerminal {
        &self.classification
    }

    pub(super) const fn cardinality(&self) -> u128 {
        self.cardinality
    }
}

/// The canonical DAG plus exact proof and gap accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CertifiedRegionLowering<Certificate> {
    case_graph: CaseDecisionDag,
    counts: CertifiedCaseCounts,
    certified_supports: Box<[CertifiedProofSupport<Certificate>]>,
    certified_cardinality: u128,
    uncovered_cardinality: u128,
}

impl<Certificate> CertifiedRegionLowering<Certificate> {
    pub(super) fn case_graph(&self) -> &CaseDecisionDag {
        &self.case_graph
    }

    pub(super) const fn counts(&self) -> CertifiedCaseCounts {
        self.counts
    }

    pub(super) fn certified_supports(&self) -> &[CertifiedProofSupport<Certificate>] {
        &self.certified_supports
    }

    pub(super) const fn certified_cardinality(&self) -> u128 {
        self.certified_cardinality
    }

    pub(super) const fn uncovered_cardinality(&self) -> u128 {
        self.uncovered_cardinality
    }

    pub(super) const fn has_uncovered_support(&self) -> bool {
        self.uncovered_cardinality != 0
    }

    pub(super) fn into_case_graph(self) -> CaseDecisionDag {
        self.case_graph
    }
}

/// Validates and lowers proof-backed finite regions without enumerating case
/// identities. Every omitted assignment becomes eligibility-open with
/// `uncovered_reason`.
pub(super) fn lower_certified_case_regions<Certificate>(
    axis_cardinalities: Vec<u128>,
    regions: impl IntoIterator<Item = CertifiedCaseRegion<Certificate>>,
    uncovered_reason: CaseOpenReason,
) -> Result<CertifiedRegionLowering<Certificate>, CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    lower_certified_case_regions_with_limits(
        axis_cardinalities,
        regions,
        uncovered_reason,
        CertifiedRegionLoweringLimits::default(),
    )
}

pub(super) fn lower_certified_case_regions_with_limits<Certificate>(
    axis_cardinalities: Vec<u128>,
    regions: impl IntoIterator<Item = CertifiedCaseRegion<Certificate>>,
    uncovered_reason: CaseOpenReason,
    limits: CertifiedRegionLoweringLimits,
) -> Result<CertifiedRegionLowering<Certificate>, CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    let universe = checked_product(&axis_cardinalities).ok_or(
        CertifiedRegionLoweringError::CardinalityOverflow {
            context: "declared case universe",
        },
    )?;
    let mut regions = regions.into_iter().collect::<Vec<_>>();
    regions.sort();

    if universe == 0 {
        if let Some(region) = regions.first() {
            return Err(CertifiedRegionLoweringError::RegionInEmptyUniverse {
                certificate: region.certificate.clone(),
            });
        }
        let case_graph = CaseDecisionDag::from_decision_partition(
            axis_cardinalities,
            DecisionPartition::empty_space(),
        )?;
        return Ok(CertifiedRegionLowering {
            case_graph,
            counts: CertifiedCaseCounts {
                universe: 0,
                excluded: 0,
                eligibility_open: 0,
                admissible_nonmatch: 0,
                admissible_match: 0,
                admissible_open: 0,
            },
            certified_supports: Vec::new().into_boxed_slice(),
            certified_cardinality: 0,
            uncovered_cardinality: 0,
        });
    }

    preflight_congruence_materialization(&axis_cardinalities, &regions, limits)?;

    let mut normalized = regions
        .into_iter()
        .map(|region| normalize_region(&axis_cardinalities, region))
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort();
    validate_disjoint(&normalized)?;

    let certified_cardinality = normalized.iter().try_fold(0_u128, |total, region| {
        total.checked_add(region.cardinality).ok_or(
            CertifiedRegionLoweringError::CardinalityOverflow {
                context: "certified region union",
            },
        )
    })?;
    let uncovered_cardinality = universe.checked_sub(certified_cardinality).ok_or(
        CertifiedRegionLoweringError::InternalInvariant(
            "disjoint certified support exceeded the declared universe",
        ),
    )?;

    let default_terminal = CaseTerminal::EligibilityOpen(uncovered_reason);
    let active = (0..normalized.len()).collect::<Vec<_>>();
    let target = build_partition_target(
        &axis_cardinalities,
        &normalized,
        &active,
        0,
        &default_terminal,
    )?;
    let case_graph = CaseDecisionDag::from_decision_partition(
        axis_cardinalities,
        DecisionPartition::target(target),
    )?;
    let counts = counts_from_graph(&case_graph)?;
    if counts.universe != universe {
        return Err(CertifiedRegionLoweringError::InternalInvariant(
            "lowered graph cardinality disagrees with the declared universe",
        ));
    }

    let mut support_by_proof = BTreeMap::new();
    for region in &normalized {
        let key = (region.certificate.clone(), region.classification.clone());
        let entry = support_by_proof.entry(key).or_insert(0_u128);
        *entry = entry.checked_add(region.cardinality).ok_or(
            CertifiedRegionLoweringError::CardinalityOverflow {
                context: "certificate support",
            },
        )?;
    }
    let certified_supports = support_by_proof
        .into_iter()
        .map(
            |((certificate, classification), cardinality)| CertifiedProofSupport {
                certificate,
                classification,
                cardinality,
            },
        )
        .collect::<Vec<_>>()
        .into_boxed_slice();

    Ok(CertifiedRegionLowering {
        case_graph,
        counts,
        certified_supports,
        certified_cardinality,
        uncovered_cardinality,
    })
}

/// Checks the artifact-wide congruence expansion budget before normalization
/// allocates any singleton ordinal interval. Counting follows canonical region,
/// source-axis and authored cell order and uses only u128 arithmetic.
fn preflight_congruence_materialization<Certificate>(
    axis_cardinalities: &[u128],
    regions: &[CertifiedCaseRegion<Certificate>],
    limits: CertifiedRegionLoweringLimits,
) -> Result<(), CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    let mut cumulative_intervals = 0_u128;
    for region in regions {
        if region.axes.len() != axis_cardinalities.len() {
            return Err(CertifiedRegionLoweringError::AxisArity {
                certificate: region.certificate.clone(),
                expected: axis_cardinalities.len(),
                actual: region.axes.len(),
            });
        }
        for (dimension, (axis, &axis_cardinality)) in
            region.axes.iter().zip(axis_cardinalities).enumerate()
        {
            for &cell in axis.cells() {
                let bounds = cell.interval_bounds();
                if bounds.end_exclusive > axis_cardinality {
                    return Err(CertifiedRegionLoweringError::AxisSupportOutOfBounds {
                        certificate: region.certificate.clone(),
                        dimension,
                        start: bounds.start,
                        end_exclusive: bounds.end_exclusive,
                        cardinality: axis_cardinality,
                    });
                }
                let CertifiedOrdinalCellKind::IntervalCongruence {
                    interval,
                    modulus,
                    residue,
                } = cell.kind
                else {
                    continue;
                };
                if modulus == 1 {
                    continue;
                }
                let Some(first) = first_congruent_at_or_after(interval.start, modulus, residue)
                else {
                    continue;
                };
                if first >= interval.end_exclusive {
                    continue;
                }
                let requested_intervals = 1 + (interval.end_exclusive - 1 - first) / modulus;
                let Some(next_cumulative) = cumulative_intervals.checked_add(requested_intervals)
                else {
                    return Err(
                        CertifiedRegionLoweringError::CongruenceMaterializationTooLarge {
                            certificate: region.certificate.clone(),
                            dimension,
                            requested_intervals,
                            cumulative_intervals: u128::MAX,
                            limit: limits.max_congruence_ordinal_intervals(),
                        },
                    );
                };
                if next_cumulative > limits.max_congruence_ordinal_intervals() {
                    return Err(
                        CertifiedRegionLoweringError::CongruenceMaterializationTooLarge {
                            certificate: region.certificate.clone(),
                            dimension,
                            requested_intervals,
                            cumulative_intervals: next_cumulative,
                            limit: limits.max_congruence_ordinal_intervals(),
                        },
                    );
                }
                cumulative_intervals = next_cumulative;
            }
        }
    }
    Ok(())
}

fn normalize_region<Certificate>(
    axis_cardinalities: &[u128],
    region: CertifiedCaseRegion<Certificate>,
) -> Result<NormalizedRegion<Certificate>, CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    if region.axes.len() != axis_cardinalities.len() {
        return Err(CertifiedRegionLoweringError::AxisArity {
            certificate: region.certificate,
            expected: axis_cardinalities.len(),
            actual: region.axes.len(),
        });
    }

    let mut cardinality = 1_u128;
    let mut axes = Vec::with_capacity(region.axes.len());
    for (dimension, (axis, &axis_cardinality)) in
        region.axes.iter().zip(axis_cardinalities).enumerate()
    {
        let normalized =
            normalize_axis_set(axis, axis_cardinality, dimension, &region.certificate)?;
        cardinality = cardinality.checked_mul(normalized.cardinality).ok_or(
            CertifiedRegionLoweringError::CardinalityOverflow {
                context: "certified region",
            },
        )?;
        axes.push(normalized);
    }

    Ok(NormalizedRegion {
        axes: axes.into_boxed_slice(),
        classification: region.classification,
        certificate: region.certificate,
        cardinality,
    })
}

fn normalize_axis_set<Certificate>(
    axis: &CertifiedAxisSet,
    axis_cardinality: u128,
    dimension: usize,
    certificate: &Certificate,
) -> Result<NormalizedAxisSet, CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    let mut intervals = Vec::new();
    for &cell in axis.cells() {
        let bounds = cell.interval_bounds();
        if bounds.end_exclusive > axis_cardinality {
            return Err(CertifiedRegionLoweringError::AxisSupportOutOfBounds {
                certificate: certificate.clone(),
                dimension,
                start: bounds.start,
                end_exclusive: bounds.end_exclusive,
                cardinality: axis_cardinality,
            });
        }
        match cell.kind {
            CertifiedOrdinalCellKind::Interval(interval) => intervals.push(interval),
            CertifiedOrdinalCellKind::IntervalCongruence {
                interval,
                modulus,
                residue,
            } => {
                let Some(first) = first_congruent_at_or_after(interval.start, modulus, residue)
                else {
                    continue;
                };
                if first >= interval.end_exclusive {
                    continue;
                }
                if modulus == 1 {
                    intervals.push(interval);
                    continue;
                }

                let selected = 1 + (interval.end_exclusive - 1 - first) / modulus;
                let reserve = usize::try_from(selected).map_err(|_| {
                    CertifiedRegionLoweringError::CongruenceMaterializationAllocationFailed {
                        certificate: certificate.clone(),
                        dimension,
                        requested_intervals: selected,
                    }
                })?;
                intervals.try_reserve_exact(reserve).map_err(|_| {
                    CertifiedRegionLoweringError::CongruenceMaterializationAllocationFailed {
                        certificate: certificate.clone(),
                        dimension,
                        requested_intervals: selected,
                    }
                })?;

                let mut ordinal = first;
                loop {
                    let end_exclusive = ordinal.checked_add(1).ok_or(
                        CertifiedRegionLoweringError::InternalInvariant(
                            "in-bounds congruence ordinal could not form a singleton interval",
                        ),
                    )?;
                    intervals.push(CertifiedOrdinalInterval {
                        start: ordinal,
                        end_exclusive,
                    });
                    let Some(next) = ordinal.checked_add(modulus) else {
                        break;
                    };
                    if next >= interval.end_exclusive {
                        break;
                    }
                    ordinal = next;
                }
            }
        }
    }

    if intervals.is_empty() {
        return Err(CertifiedRegionLoweringError::EmptyAxisSupport {
            certificate: certificate.clone(),
            dimension,
        });
    }
    intervals.sort();
    let mut normalized: Vec<CertifiedOrdinalInterval> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        if let Some(last) = normalized.last_mut() {
            if interval.start <= last.end_exclusive {
                last.end_exclusive = last.end_exclusive.max(interval.end_exclusive);
                continue;
            }
        }
        normalized.push(interval);
    }
    let cardinality = normalized.iter().try_fold(0_u128, |total, interval| {
        total
            .checked_add(interval.len())
            .ok_or(CertifiedRegionLoweringError::CardinalityOverflow {
                context: "certified axis set",
            })
    })?;
    Ok(NormalizedAxisSet {
        intervals: normalized.into_boxed_slice(),
        cardinality,
    })
}

fn first_congruent_at_or_after(start: u128, modulus: u128, residue: u128) -> Option<u128> {
    let start_residue = start % modulus;
    let delta = if start_residue <= residue {
        residue - start_residue
    } else {
        modulus - (start_residue - residue)
    };
    start.checked_add(delta)
}

fn validate_disjoint<Certificate>(
    regions: &[NormalizedRegion<Certificate>],
) -> Result<(), CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    for left in 0..regions.len() {
        for right in left + 1..regions.len() {
            let lhs = &regions[left];
            let rhs = &regions[right];
            if lhs
                .axes
                .iter()
                .zip(rhs.axes.iter())
                .all(|(left_axis, right_axis)| left_axis.intersects(right_axis))
            {
                if lhs.classification == rhs.classification {
                    return Err(CertifiedRegionLoweringError::OverlappingRegions {
                        first_certificate: lhs.certificate.clone(),
                        second_certificate: rhs.certificate.clone(),
                        classification: lhs.classification.clone(),
                    });
                }
                return Err(CertifiedRegionLoweringError::ConflictingRegions {
                    first_certificate: lhs.certificate.clone(),
                    second_certificate: rhs.certificate.clone(),
                    first_classification: lhs.classification.clone(),
                    second_classification: rhs.classification.clone(),
                });
            }
        }
    }
    Ok(())
}

fn build_partition_target<Certificate>(
    axis_cardinalities: &[u128],
    regions: &[NormalizedRegion<Certificate>],
    active: &[usize],
    dimension: usize,
    default_terminal: &CaseTerminal,
) -> Result<DecisionPartitionTarget<CaseTerminal>, CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    if active.is_empty() {
        return Ok(DecisionPartitionTarget::terminal(default_terminal.clone()));
    }
    if dimension == axis_cardinalities.len() {
        if active.len() != 1 {
            return Err(CertifiedRegionLoweringError::InternalInvariant(
                "overlapping regions reached a partition leaf",
            ));
        }
        return Ok(DecisionPartitionTarget::terminal(
            regions[active[0]].classification.clone(),
        ));
    }

    let mut cuts = BTreeSet::from([0, axis_cardinalities[dimension]]);
    for &region_index in active {
        for interval in regions[region_index].axes[dimension].intervals.iter() {
            cuts.insert(interval.start);
            cuts.insert(interval.end_exclusive);
        }
    }
    let cuts = cuts.into_iter().collect::<Vec<_>>();
    let mut intervals_by_active = BTreeMap::<Vec<usize>, Vec<(u128, u128)>>::new();
    for pair in cuts.windows(2) {
        let start = pair[0];
        let end_exclusive = pair[1];
        if start == end_exclusive {
            continue;
        }
        let next_active = active
            .iter()
            .copied()
            .filter(|&region_index| regions[region_index].axes[dimension].contains(start))
            .collect::<Vec<_>>();
        intervals_by_active
            .entry(next_active)
            .or_default()
            .push((start, end_exclusive));
    }

    let mut arcs = Vec::with_capacity(intervals_by_active.len());
    for (next_active, intervals) in intervals_by_active {
        let child = build_partition_target(
            axis_cardinalities,
            regions,
            &next_active,
            dimension + 1,
            default_terminal,
        )?;
        arcs.push(DecisionPartitionArc::new(intervals, child)?);
    }
    Ok(DecisionPartitionTarget::decision(dimension, arcs)?)
}

fn counts_from_graph<Certificate>(
    graph: &CaseDecisionDag,
) -> Result<CertifiedCaseCounts, CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    let mut counts = CertifiedCaseCounts {
        universe: 0,
        excluded: 0,
        eligibility_open: 0,
        admissible_nonmatch: 0,
        admissible_match: 0,
        admissible_open: 0,
    };
    for (terminal, cardinality) in graph.terminal_counts()? {
        let CheckedCardinality::Exact(cardinality) = cardinality else {
            return Err(CertifiedRegionLoweringError::CardinalityOverflow {
                context: "lowered graph terminal multiplicity",
            });
        };
        counts.universe = checked_add(counts.universe, cardinality, "lowered universe")?;
        let target = match terminal {
            CaseTerminal::Excluded => &mut counts.excluded,
            CaseTerminal::EligibilityOpen(_) => &mut counts.eligibility_open,
            CaseTerminal::AdmissibleNonmatch => &mut counts.admissible_nonmatch,
            CaseTerminal::AdmissibleMatch => &mut counts.admissible_match,
            CaseTerminal::AdmissibleOpen(_) => &mut counts.admissible_open,
        };
        *target = checked_add(*target, cardinality, "terminal class")?;
    }
    Ok(counts)
}

fn checked_add<Certificate>(
    left: u128,
    right: u128,
    context: &'static str,
) -> Result<u128, CertifiedRegionLoweringError<Certificate>>
where
    Certificate: Clone + Ord,
{
    left.checked_add(right)
        .ok_or(CertifiedRegionLoweringError::CardinalityOverflow { context })
}

fn checked_product(factors: &[u128]) -> Option<u128> {
    if factors.contains(&0) {
        return Some(0);
    }
    factors
        .iter()
        .try_fold(1_u128, |product, &factor| product.checked_mul(factor))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CertifiedRegionLoweringError<Certificate> {
    AxisArity {
        certificate: Certificate,
        expected: usize,
        actual: usize,
    },
    AxisSupportOutOfBounds {
        certificate: Certificate,
        dimension: usize,
        start: u128,
        end_exclusive: u128,
        cardinality: u128,
    },
    EmptyAxisSupport {
        certificate: Certificate,
        dimension: usize,
    },
    RegionInEmptyUniverse {
        certificate: Certificate,
    },
    CongruenceMaterializationTooLarge {
        certificate: Certificate,
        dimension: usize,
        requested_intervals: u128,
        cumulative_intervals: u128,
        limit: u128,
    },
    CongruenceMaterializationAllocationFailed {
        certificate: Certificate,
        dimension: usize,
        requested_intervals: u128,
    },
    OverlappingRegions {
        first_certificate: Certificate,
        second_certificate: Certificate,
        classification: CaseTerminal,
    },
    ConflictingRegions {
        first_certificate: Certificate,
        second_certificate: Certificate,
        first_classification: CaseTerminal,
        second_classification: CaseTerminal,
    },
    CardinalityOverflow {
        context: &'static str,
    },
    CaseGraph(CaseGraphError),
    InternalInvariant(&'static str),
}

impl<Certificate> From<CaseGraphError> for CertifiedRegionLoweringError<Certificate> {
    fn from(error: CaseGraphError) -> Self {
        Self::CaseGraph(error)
    }
}

impl<Certificate: fmt::Debug> fmt::Display for CertifiedRegionLoweringError<Certificate> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AxisArity {
                certificate,
                expected,
                actual,
            } => write!(
                formatter,
                "certificate {certificate:?} has {actual} axes, expected {expected}"
            ),
            Self::AxisSupportOutOfBounds {
                certificate,
                dimension,
                start,
                end_exclusive,
                cardinality,
            } => write!(
                formatter,
                "certificate {certificate:?} interval [{start}, {end_exclusive}) is outside dimension {dimension} cardinality {cardinality}"
            ),
            Self::EmptyAxisSupport {
                certificate,
                dimension,
            } => write!(
                formatter,
                "certificate {certificate:?} has empty support on dimension {dimension}"
            ),
            Self::RegionInEmptyUniverse { certificate } => write!(
                formatter,
                "certificate {certificate:?} supplies a region in an empty case universe"
            ),
            Self::CongruenceMaterializationTooLarge {
                certificate,
                dimension,
                requested_intervals,
                cumulative_intervals,
                limit,
            } => write!(
                formatter,
                "certificate {certificate:?} requests {requested_intervals} congruence ordinal intervals on dimension {dimension}, bringing the artifact total to {cumulative_intervals} above the deterministic limit {limit}"
            ),
            Self::CongruenceMaterializationAllocationFailed {
                certificate,
                dimension,
                requested_intervals,
            } => write!(
                formatter,
                "could not allocate {requested_intervals} preflight-approved congruence ordinal intervals for certificate {certificate:?} on dimension {dimension}"
            ),
            Self::OverlappingRegions {
                first_certificate,
                second_certificate,
                classification,
            } => write!(
                formatter,
                "certificates {first_certificate:?} and {second_certificate:?} overlap with classification {classification:?}"
            ),
            Self::ConflictingRegions {
                first_certificate,
                second_certificate,
                first_classification,
                second_classification,
            } => write!(
                formatter,
                "certificates {first_certificate:?} ({first_classification:?}) and {second_certificate:?} ({second_classification:?}) overlap with conflicting classifications"
            ),
            Self::CardinalityOverflow { context } => {
                write!(formatter, "{context} cardinality exceeds u128::MAX")
            }
            Self::CaseGraph(error) => write!(formatter, "cannot construct certified case graph: {error}"),
            Self::InternalInvariant(message) => {
                write!(formatter, "certified region lowering invariant failed: {message}")
            }
        }
    }
}

impl<Certificate: fmt::Debug> Error for CertifiedRegionLoweringError<Certificate> {}

#[cfg(test)]
mod tests {
    use super::super::case_graph::{DecisionRef, DecisionRoot};
    use super::*;

    fn interval(start: u128, end_exclusive: u128) -> CertifiedOrdinalInterval {
        CertifiedOrdinalInterval::new(start, end_exclusive).unwrap()
    }

    fn rectangle(
        axes: &[(u128, u128)],
        classification: CaseTerminal,
        certificate: &'static str,
    ) -> CertifiedCaseRegion<&'static str> {
        CertifiedCaseRegion::rectangle(
            axes.iter().map(|&(start, end)| interval(start, end)),
            classification,
            certificate,
        )
    }

    fn lower(
        axes: Vec<u128>,
        regions: Vec<CertifiedCaseRegion<&'static str>>,
    ) -> Result<CertifiedRegionLowering<&'static str>, CertifiedRegionLoweringError<&'static str>>
    {
        lower_certified_case_regions(axes, regions, CaseOpenReason::EvaluationUnknown)
    }

    fn lower_with_congruence_limit(
        axes: Vec<u128>,
        regions: Vec<CertifiedCaseRegion<&'static str>>,
        limit: u128,
    ) -> Result<CertifiedRegionLowering<&'static str>, CertifiedRegionLoweringError<&'static str>>
    {
        lower_certified_case_regions_with_limits(
            axes,
            regions,
            CaseOpenReason::EvaluationUnknown,
            CertifiedRegionLoweringLimits::new(limit),
        )
    }

    #[test]
    fn disconnected_regions_share_one_terminal_without_widening_the_gap() {
        let lowering = lower(
            vec![8],
            vec![
                rectangle(&[(0, 2)], CaseTerminal::AdmissibleMatch, "left"),
                rectangle(&[(5, 7)], CaseTerminal::AdmissibleMatch, "right"),
            ],
        )
        .unwrap();

        assert_eq!(lowering.counts().universe(), 8);
        assert_eq!(lowering.counts().admissible_match(), 4);
        assert_eq!(lowering.counts().eligibility_open(), 4);
        assert_eq!(lowering.uncovered_cardinality(), 4);
        let DecisionRoot::Target(DecisionRef::Node(root)) = lowering.case_graph().root() else {
            panic!("disconnected support requires one decision node");
        };
        let match_arc = lowering
            .case_graph()
            .node(root)
            .unwrap()
            .arcs()
            .iter()
            .find(|arc| match arc.child() {
                DecisionRef::Terminal(id) => {
                    lowering.case_graph().terminal(id) == Some(&CaseTerminal::AdmissibleMatch)
                }
                DecisionRef::Node(_) => false,
            })
            .unwrap();
        assert_eq!(
            match_arc
                .ordinals()
                .intervals()
                .iter()
                .map(|interval| (interval.start().get(), interval.end_exclusive().get()))
                .collect::<Vec<_>>(),
            vec![(0, 2), (5, 7)]
        );
    }

    #[test]
    fn correlated_nonrectangular_union_never_cartesianizes_marginals() {
        let lowering = lower(
            vec![2, 2],
            vec![
                rectangle(
                    &[(0, 1), (0, 1)],
                    CaseTerminal::AdmissibleMatch,
                    "northwest",
                ),
                rectangle(
                    &[(1, 2), (1, 2)],
                    CaseTerminal::AdmissibleMatch,
                    "southeast",
                ),
            ],
        )
        .unwrap();

        for path in [[0, 0], [1, 1]] {
            assert_eq!(
                lowering.case_graph().terminal_for_path(&path).unwrap(),
                Some(&CaseTerminal::AdmissibleMatch)
            );
        }
        for path in [[0, 1], [1, 0]] {
            assert_eq!(
                lowering.case_graph().terminal_for_path(&path).unwrap(),
                Some(&CaseTerminal::EligibilityOpen(
                    CaseOpenReason::EvaluationUnknown
                ))
            );
        }
        assert_eq!(lowering.counts().admissible_match(), 2);
        assert_eq!(lowering.uncovered_cardinality(), 2);
    }

    #[test]
    fn uncovered_complement_is_counted_and_stays_eligibility_open() {
        let lowering = lower(
            vec![10],
            vec![rectangle(
                &[(2, 5)],
                CaseTerminal::AdmissibleNonmatch,
                "middle",
            )],
        )
        .unwrap();

        assert_eq!(lowering.certified_cardinality(), 3);
        assert_eq!(lowering.uncovered_cardinality(), 7);
        assert!(lowering.has_uncovered_support());
        assert_eq!(lowering.counts().admissible_nonmatch(), 3);
        assert_eq!(lowering.counts().eligibility_open(), 7);
        assert_eq!(
            lowering.counts().admissible_configurations(),
            CertifiedClosureCount::LowerBound(3)
        );
        assert_eq!(
            lowering.counts().matching_configurations(),
            CertifiedClosureCount::LowerBound(0)
        );
    }

    #[test]
    fn weighted_u_d_m_and_both_open_layers_are_exact_terminal_counts() {
        let lowering = lower(
            vec![10],
            vec![
                rectangle(&[(0, 1)], CaseTerminal::Excluded, "excluded"),
                rectangle(&[(1, 3)], CaseTerminal::AdmissibleNonmatch, "nonmatch"),
                rectangle(&[(3, 6)], CaseTerminal::AdmissibleMatch, "match"),
                rectangle(
                    &[(6, 8)],
                    CaseTerminal::AdmissibleOpen(CaseOpenReason::SearchBudgetExhausted),
                    "polarity-open",
                ),
            ],
        )
        .unwrap();

        let counts = lowering.counts();
        assert_eq!(counts.universe(), 10);
        assert_eq!(counts.excluded(), 1);
        assert_eq!(counts.admissible_nonmatch(), 2);
        assert_eq!(counts.admissible_match(), 3);
        assert_eq!(counts.admissible_open(), 2);
        assert_eq!(counts.eligibility_open(), 2);
        assert_eq!(counts.known_admissible(), 7);
        assert_eq!(
            counts.admissible_configurations(),
            CertifiedClosureCount::LowerBound(7)
        );
        assert_eq!(
            counts.matching_configurations(),
            CertifiedClosureCount::LowerBound(3)
        );
    }

    #[test]
    fn same_terminal_overlap_and_terminal_conflict_are_distinct_errors() {
        let overlap = lower(
            vec![8],
            vec![
                rectangle(&[(0, 5)], CaseTerminal::AdmissibleMatch, "a"),
                rectangle(&[(4, 7)], CaseTerminal::AdmissibleMatch, "b"),
            ],
        )
        .unwrap_err();
        assert!(matches!(
            overlap,
            CertifiedRegionLoweringError::OverlappingRegions { .. }
        ));

        let conflict = lower(
            vec![8],
            vec![
                rectangle(&[(0, 5)], CaseTerminal::AdmissibleMatch, "a"),
                rectangle(&[(4, 7)], CaseTerminal::AdmissibleNonmatch, "b"),
            ],
        )
        .unwrap_err();
        assert!(matches!(
            conflict,
            CertifiedRegionLoweringError::ConflictingRegions { .. }
        ));
    }

    #[test]
    fn empty_axis_has_distinguished_empty_graph_and_zero_counts() {
        let lowering = lower(vec![3, 0, 2], Vec::new()).unwrap();

        assert_eq!(lowering.case_graph().root(), DecisionRoot::EmptySpace);
        assert_eq!(lowering.counts().universe(), 0);
        assert_eq!(lowering.uncovered_cardinality(), 0);
        assert!(lowering.case_graph().nodes().is_empty());
        assert!(lowering.case_graph().terminals().is_empty());
    }

    #[test]
    fn congruence_cell_lowers_to_exact_noncontiguous_ordinal_support() {
        let congruence = CertifiedAxisSet::interval_congruence(interval(1, 11), 3, 1).unwrap();
        let lowering = lower(
            vec![12],
            vec![CertifiedCaseRegion::new(
                vec![congruence],
                CaseTerminal::AdmissibleMatch,
                "mod-three",
            )],
        )
        .unwrap();

        for ordinal in 0..12 {
            let expected = if [1, 4, 7, 10].contains(&ordinal) {
                CaseTerminal::AdmissibleMatch
            } else {
                CaseTerminal::EligibilityOpen(CaseOpenReason::EvaluationUnknown)
            };
            assert_eq!(
                lowering.case_graph().terminal_for_path(&[ordinal]).unwrap(),
                Some(&expected)
            );
        }
        assert_eq!(lowering.counts().admissible_match(), 4);
        assert_eq!(lowering.uncovered_cardinality(), 8);
    }

    #[test]
    fn one_congruence_cell_cannot_exceed_the_deterministic_materialization_cap() {
        let congruence = CertifiedAxisSet::interval_congruence(interval(0, 8), 2, 0).unwrap();
        let error = lower_with_congruence_limit(
            vec![8],
            vec![CertifiedCaseRegion::new(
                vec![congruence],
                CaseTerminal::AdmissibleMatch,
                "four-even-ordinals",
            )],
            3,
        )
        .unwrap_err();

        assert_eq!(
            error,
            CertifiedRegionLoweringError::CongruenceMaterializationTooLarge {
                certificate: "four-even-ordinals",
                dimension: 0,
                requested_intervals: 4,
                cumulative_intervals: 4,
                limit: 3,
            }
        );
    }

    #[test]
    fn congruence_materialization_cap_is_cumulative_across_axis_cells() {
        let even = CertifiedOrdinalCell::interval_congruence(interval(0, 6), 2, 0).unwrap();
        let odd = CertifiedOrdinalCell::interval_congruence(interval(0, 6), 2, 1).unwrap();
        let axis = CertifiedAxisSet::new(vec![even, odd]).unwrap();
        let error = lower_with_congruence_limit(
            vec![6],
            vec![CertifiedCaseRegion::new(
                vec![axis],
                CaseTerminal::AdmissibleMatch,
                "two-residue-cells",
            )],
            5,
        )
        .unwrap_err();

        assert_eq!(
            error,
            CertifiedRegionLoweringError::CongruenceMaterializationTooLarge {
                certificate: "two-residue-cells",
                dimension: 0,
                requested_intervals: 3,
                cumulative_intervals: 6,
                limit: 5,
            }
        );
    }

    #[test]
    fn huge_rectangular_cardinality_is_weighted_without_case_enumeration() {
        let side = u64::MAX as u128;
        let expected = side.checked_mul(side).unwrap();
        let lowering = lower(
            vec![side, side],
            vec![rectangle(
                &[(0, side), (0, side)],
                CaseTerminal::AdmissibleMatch,
                "whole-universe",
            )],
        )
        .unwrap();

        assert_eq!(lowering.counts().universe(), expected);
        assert_eq!(lowering.counts().admissible_match(), expected);
        assert_eq!(lowering.uncovered_cardinality(), 0);
        assert_eq!(
            lowering.counts().matching_configurations(),
            CertifiedClosureCount::Exact(expected)
        );
        let DecisionRoot::Target(DecisionRef::Terminal(_)) = lowering.case_graph().root() else {
            panic!("a uniform huge universe should reduce to one terminal");
        };
    }

    #[test]
    fn canonical_graph_does_not_depend_on_region_discovery_order() {
        let left = rectangle(&[(0, 2)], CaseTerminal::AdmissibleMatch, "left");
        let right = rectangle(&[(5, 7)], CaseTerminal::AdmissibleNonmatch, "right");
        let forward = lower(vec![8], vec![left.clone(), right.clone()]).unwrap();
        let reverse = lower(vec![8], vec![right, left]).unwrap();

        assert_eq!(forward.case_graph(), reverse.case_graph());
        assert_eq!(forward.certified_supports(), reverse.certified_supports());
    }
}
