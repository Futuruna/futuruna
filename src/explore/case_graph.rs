//! Canonical case-classification decision diagrams for bounded exploration.
//!
//! The builder consumes cases in mixed-radix order (the last declared axis is
//! fastest) and reduces completed suffixes immediately. It therefore retains
//! the decision DAG and one active frame per axis, rather than a ledger of all
//! classified paths. A partial build can classify the untouched canonical
//! suffix with one explicit open terminal without enumerating that suffix.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

/// One zero-based position in a declared finite domain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct DomainOrdinal(pub(super) u128);

impl DomainOrdinal {
    pub(super) const fn get(self) -> u128 {
        self.0
    }
}

/// A canonical assignment to every independently varied axis.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct CaseOrdinalPath(Vec<DomainOrdinal>);

impl CaseOrdinalPath {
    pub(super) fn as_slice(&self) -> &[DomainOrdinal] {
        &self.0
    }

    pub(super) fn to_raw_ordinals(&self) -> Vec<u128> {
        self.0.iter().map(|ordinal| ordinal.0).collect()
    }

    fn matches_raw(&self, ordinals: &[u128]) -> bool {
        self.0.len() == ordinals.len()
            && self
                .0
                .iter()
                .zip(ordinals)
                .all(|(expected, actual)| expected.0 == *actual)
    }
}

impl From<Vec<u128>> for CaseOrdinalPath {
    fn from(ordinals: Vec<u128>) -> Self {
        Self(ordinals.into_iter().map(DomainOrdinal).collect())
    }
}

/// Why one portion of the finite case space remains open.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CaseOpenReason {
    SearchBudgetExhausted,
    EvaluationUnknown,
}

/// The total evidence classification represented by a case decision DAG.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CaseTerminal {
    Excluded,
    EligibilityOpen(CaseOpenReason),
    AdmissibleNonmatch,
    AdmissibleMatch,
    AdmissibleOpen(CaseOpenReason),
}

pub(crate) type CaseDecisionDag = OrderedDecisionDag<CaseTerminal>;
pub(super) type CaseGraphBuilder = OrderedDecisionDagBuilder<CaseTerminal>;

pub(super) const DEFAULT_MAX_CASE_RANK_RUN_AXES: usize = 256;
pub(super) const DEFAULT_MAX_CASE_RANK_RUNS: usize = 65_536;
pub(super) const DEFAULT_MAX_CASE_RANK_RUN_NODES: usize = 131_072;
pub(super) const DEFAULT_MAX_CASE_RANK_RUN_ARCS: usize = 262_144;
pub(super) const DEFAULT_MAX_CASE_RANK_RUN_ORDINAL_INTERVALS: usize = 262_144;
pub(super) const DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES: usize = 64 * 1024 * 1024;

// Platform-independent upper weights account for each payload together with
// vector slack and interner/map overhead. This is deliberately conservative
// work accounting, not a claim about the allocator's exact resident bytes.
const ACCOUNTED_AXIS_BYTES: usize = 16;
const ACCOUNTED_STRIDE_BYTES: usize = 16;
const ACCOUNTED_INPUT_RUN_BYTES: usize = 64;
const ACCOUNTED_NORMALIZED_SEGMENT_BYTES: usize = 96;
const ACCOUNTED_CUT_BYTES: usize = 16;
const ACCOUNTED_TERMINAL_BYTES: usize = 64;
const ACCOUNTED_NODE_BYTES: usize = 256;
const ACCOUNTED_ARC_BYTES: usize = 192;
const ACCOUNTED_ORDINAL_INTERVAL_BYTES: usize = 96;

/// One nonempty uniform classification in canonical mixed-radix rank space.
///
/// Construction deliberately does not validate the endpoints. The bounded
/// lowerer validates the complete sorted stream before any run becomes case
/// evidence, which also keeps malformed-input tests at the public seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CaseTerminalRankRun {
    start: u128,
    end_exclusive: u128,
    terminal: CaseTerminal,
}

impl CaseTerminalRankRun {
    pub(super) const fn new(start: u128, end_exclusive: u128, terminal: CaseTerminal) -> Self {
        Self {
            start,
            end_exclusive,
            terminal,
        }
    }

    pub(super) const fn start(&self) -> u128 {
        self.start
    }

    pub(super) const fn end_exclusive(&self) -> u128 {
        self.end_exclusive
    }

    pub(super) const fn terminal(&self) -> &CaseTerminal {
        &self.terminal
    }
}

/// Operational materialization limits for lowering uniform rank runs.
///
/// These limits do not enter graph identity. Exceeding one returns no graph;
/// callers must report unavailable bounded evidence rather than a partial or
/// zero-valued case DAG.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CaseRankRunLoweringLimits {
    max_axes: usize,
    max_runs: usize,
    max_nodes: usize,
    max_arcs: usize,
    max_ordinal_intervals: usize,
    max_accounted_bytes: usize,
}

impl CaseRankRunLoweringLimits {
    pub(super) const fn new(
        max_axes: usize,
        max_runs: usize,
        max_nodes: usize,
        max_arcs: usize,
        max_ordinal_intervals: usize,
        max_accounted_bytes: usize,
    ) -> Self {
        Self {
            max_axes,
            max_runs,
            max_nodes,
            max_arcs,
            max_ordinal_intervals,
            max_accounted_bytes,
        }
    }

    pub(super) const fn max_axes(self) -> usize {
        self.max_axes
    }

    pub(super) const fn max_runs(self) -> usize {
        self.max_runs
    }

    pub(super) const fn max_nodes(self) -> usize {
        self.max_nodes
    }

    pub(super) const fn max_arcs(self) -> usize {
        self.max_arcs
    }

    pub(super) const fn max_ordinal_intervals(self) -> usize {
        self.max_ordinal_intervals
    }

    pub(super) const fn max_accounted_bytes(self) -> usize {
        self.max_accounted_bytes
    }
}

impl Default for CaseRankRunLoweringLimits {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_CASE_RANK_RUN_AXES,
            DEFAULT_MAX_CASE_RANK_RUNS,
            DEFAULT_MAX_CASE_RANK_RUN_NODES,
            DEFAULT_MAX_CASE_RANK_RUN_ARCS,
            DEFAULT_MAX_CASE_RANK_RUN_ORDINAL_INTERVALS,
            DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CaseRankRunLoweringResource {
    Axes,
    Runs,
    Nodes,
    Arcs,
    OrdinalIntervals,
    AccountedBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CaseRankRunLoweringError {
    InvalidOpenComplement,
    UniverseCardinalityOverflow,
    RunInEmptyUniverse {
        index: usize,
    },
    EmptyOrReversedRun {
        index: usize,
        start: u128,
        end_exclusive: u128,
    },
    UnsortedOrOverlappingRun {
        index: usize,
        previous_end_exclusive: u128,
        start: u128,
    },
    RunOutOfBounds {
        index: usize,
        end_exclusive: u128,
        universe: u128,
    },
    LimitExceeded {
        resource: CaseRankRunLoweringResource,
        observed: usize,
        limit: usize,
    },
    AllocationFailed {
        resource: CaseRankRunLoweringResource,
    },
    ArithmeticOverflow(&'static str),
    TerminalCountOverflow,
    TerminalCountMismatch {
        expected: BTreeMap<CaseTerminal, u128>,
        actual: BTreeMap<CaseTerminal, u128>,
    },
    Graph(CaseGraphError),
    InternalInvariant(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizedCaseTerminalRankSegment {
    start: u128,
    end_exclusive: u128,
    terminal: CaseTerminal,
}

struct CaseRankRunLoweringBudget {
    limits: CaseRankRunLoweringLimits,
    runs: usize,
    nodes: usize,
    arcs: usize,
    ordinal_intervals: usize,
    accounted_bytes: usize,
}

impl CaseRankRunLoweringBudget {
    fn new(limits: CaseRankRunLoweringLimits) -> Self {
        Self {
            limits,
            runs: 0,
            nodes: 0,
            arcs: 0,
            ordinal_intervals: 0,
            accounted_bytes: 0,
        }
    }

    fn charge_axes(&mut self, axes: usize) -> Result<(), CaseRankRunLoweringError> {
        if axes > self.limits.max_axes() {
            return Err(CaseRankRunLoweringError::LimitExceeded {
                resource: CaseRankRunLoweringResource::Axes,
                observed: axes,
                limit: self.limits.max_axes(),
            });
        }
        self.charge_scaled_bytes(axes, ACCOUNTED_AXIS_BYTES, "axis byte accounting")
    }

    fn charge_strides(&mut self, strides: usize) -> Result<(), CaseRankRunLoweringError> {
        self.charge_scaled_bytes(strides, ACCOUNTED_STRIDE_BYTES, "stride byte accounting")
    }

    fn charge_run(&mut self) -> Result<(), CaseRankRunLoweringError> {
        Self::charge_counter(
            &mut self.runs,
            1,
            self.limits.max_runs(),
            CaseRankRunLoweringResource::Runs,
        )?;
        self.charge_bytes(ACCOUNTED_INPUT_RUN_BYTES)
    }

    fn charge_segment(&mut self) -> Result<(), CaseRankRunLoweringError> {
        self.charge_bytes(ACCOUNTED_NORMALIZED_SEGMENT_BYTES)
    }

    fn charge_cuts(&mut self, cuts: usize) -> Result<(), CaseRankRunLoweringError> {
        self.charge_scaled_bytes(cuts, ACCOUNTED_CUT_BYTES, "cut byte accounting")
    }

    fn charge_terminal(&mut self) -> Result<(), CaseRankRunLoweringError> {
        self.charge_bytes(ACCOUNTED_TERMINAL_BYTES)
    }

    fn charge_node_construction(&mut self) -> Result<(), CaseRankRunLoweringError> {
        self.charge_bytes(ACCOUNTED_NODE_BYTES)
    }

    fn charge_unique_node(&mut self) -> Result<(), CaseRankRunLoweringError> {
        Self::charge_counter(
            &mut self.nodes,
            1,
            self.limits.max_nodes(),
            CaseRankRunLoweringResource::Nodes,
        )
    }

    fn charge_arc_construction(&mut self, arcs: usize) -> Result<(), CaseRankRunLoweringError> {
        self.charge_scaled_bytes(arcs, ACCOUNTED_ARC_BYTES, "arc byte accounting")
    }

    fn charge_unique_arcs(&mut self, arcs: usize) -> Result<(), CaseRankRunLoweringError> {
        Self::charge_counter(
            &mut self.arcs,
            arcs,
            self.limits.max_arcs(),
            CaseRankRunLoweringResource::Arcs,
        )
    }

    fn charge_ordinal_interval_construction(&mut self) -> Result<(), CaseRankRunLoweringError> {
        self.charge_bytes(ACCOUNTED_ORDINAL_INTERVAL_BYTES)
    }

    fn charge_unique_ordinal_intervals(
        &mut self,
        intervals: usize,
    ) -> Result<(), CaseRankRunLoweringError> {
        Self::charge_counter(
            &mut self.ordinal_intervals,
            intervals,
            self.limits.max_ordinal_intervals(),
            CaseRankRunLoweringResource::OrdinalIntervals,
        )
    }

    fn charge_scaled_bytes(
        &mut self,
        count: usize,
        width: usize,
        context: &'static str,
    ) -> Result<(), CaseRankRunLoweringError> {
        let bytes = count
            .checked_mul(width)
            .ok_or(CaseRankRunLoweringError::ArithmeticOverflow(context))?;
        self.charge_bytes(bytes)
    }

    fn charge_bytes(&mut self, bytes: usize) -> Result<(), CaseRankRunLoweringError> {
        Self::charge_counter(
            &mut self.accounted_bytes,
            bytes,
            self.limits.max_accounted_bytes(),
            CaseRankRunLoweringResource::AccountedBytes,
        )
    }

    fn charge_counter(
        current: &mut usize,
        amount: usize,
        limit: usize,
        resource: CaseRankRunLoweringResource,
    ) -> Result<(), CaseRankRunLoweringError> {
        let Some(next) = current.checked_add(amount) else {
            return Err(CaseRankRunLoweringError::LimitExceeded {
                resource,
                observed: usize::MAX,
                limit,
            });
        };
        if next > limit {
            return Err(CaseRankRunLoweringError::LimitExceeded {
                resource,
                observed: next,
                limit,
            });
        }
        *current = next;
        Ok(())
    }
}

/// Lower sorted, disjoint uniform rank runs plus an explicit open complement
/// into the canonical case decision DAG without enumerating singleton ranks.
pub(super) fn lower_case_terminal_rank_runs<I>(
    axis_cardinalities: Vec<u128>,
    runs: I,
    open_complement: CaseTerminal,
) -> Result<CaseDecisionDag, CaseRankRunLoweringError>
where
    I: IntoIterator<Item = CaseTerminalRankRun>,
{
    lower_case_terminal_rank_runs_with_limits(
        axis_cardinalities,
        runs,
        open_complement,
        CaseRankRunLoweringLimits::default(),
    )
}

pub(super) fn lower_case_terminal_rank_runs_with_limits<I>(
    axis_cardinalities: Vec<u128>,
    runs: I,
    open_complement: CaseTerminal,
    limits: CaseRankRunLoweringLimits,
) -> Result<CaseDecisionDag, CaseRankRunLoweringError>
where
    I: IntoIterator<Item = CaseTerminalRankRun>,
{
    if !matches!(
        &open_complement,
        CaseTerminal::EligibilityOpen(_) | CaseTerminal::AdmissibleOpen(_)
    ) {
        return Err(CaseRankRunLoweringError::InvalidOpenComplement);
    }

    let mut budget = CaseRankRunLoweringBudget::new(limits);
    budget.charge_axes(axis_cardinalities.len())?;

    if axis_cardinalities.contains(&0) {
        for (index, _) in runs.into_iter().enumerate() {
            budget.charge_run()?;
            return Err(CaseRankRunLoweringError::RunInEmptyUniverse { index });
        }
        let graph = CaseDecisionDag {
            axis_cardinalities,
            root: DecisionRoot::EmptySpace,
            nodes: Vec::new(),
            terminals: Vec::new(),
        };
        graph.validate()?;
        return Ok(graph);
    }

    let stride_count = axis_cardinalities.len().checked_add(1).ok_or(
        CaseRankRunLoweringError::ArithmeticOverflow("mixed-radix stride count"),
    )?;
    budget.charge_strides(stride_count)?;
    let mut suffix_strides = Vec::new();
    suffix_strides
        .try_reserve_exact(stride_count)
        .map_err(|_| CaseRankRunLoweringError::AllocationFailed {
            resource: CaseRankRunLoweringResource::AccountedBytes,
        })?;
    suffix_strides.resize(stride_count, 1_u128);
    for dimension in (0..axis_cardinalities.len()).rev() {
        suffix_strides[dimension] = axis_cardinalities[dimension]
            .checked_mul(suffix_strides[dimension + 1])
            .ok_or(CaseRankRunLoweringError::UniverseCardinalityOverflow)?;
    }
    let universe = suffix_strides[0];

    let mut segments = Vec::<NormalizedCaseTerminalRankSegment>::new();
    let mut previous_end_exclusive = 0_u128;
    for (index, run) in runs.into_iter().enumerate() {
        budget.charge_run()?;
        if run.start >= run.end_exclusive {
            return Err(CaseRankRunLoweringError::EmptyOrReversedRun {
                index,
                start: run.start,
                end_exclusive: run.end_exclusive,
            });
        }
        if run.start < previous_end_exclusive {
            return Err(CaseRankRunLoweringError::UnsortedOrOverlappingRun {
                index,
                previous_end_exclusive,
                start: run.start,
            });
        }
        if run.end_exclusive > universe {
            return Err(CaseRankRunLoweringError::RunOutOfBounds {
                index,
                end_exclusive: run.end_exclusive,
                universe,
            });
        }
        if previous_end_exclusive < run.start {
            append_normalized_rank_segment(
                &mut segments,
                previous_end_exclusive,
                run.start,
                open_complement.clone(),
                &mut budget,
            )?;
        }
        append_normalized_rank_segment(
            &mut segments,
            run.start,
            run.end_exclusive,
            run.terminal,
            &mut budget,
        )?;
        previous_end_exclusive = run.end_exclusive;
    }
    if previous_end_exclusive < universe {
        append_normalized_rank_segment(
            &mut segments,
            previous_end_exclusive,
            universe,
            open_complement,
            &mut budget,
        )?;
    }
    if segments.is_empty() {
        return Err(CaseRankRunLoweringError::InternalInvariant(
            "nonempty rank universe normalized to no terminal segments",
        ));
    }

    let expected_counts = terminal_counts_from_rank_segments(&segments, universe)?;
    let mut builder = DecisionPartitionDagBuilder::new(axis_cardinalities);
    let root = builder.build_rank_segment_target(
        &segments,
        0,
        segments.len(),
        &suffix_strides,
        0,
        0,
        &mut budget,
    )?;
    let graph = CaseDecisionDag {
        axis_cardinalities: builder.axis_cardinalities,
        root: DecisionRoot::Target(root),
        nodes: builder.nodes,
        terminals: builder.terminals,
    };
    graph.validate()?;
    validate_rank_run_graph_shape(&graph, limits)?;

    let actual_counts = graph
        .terminal_counts()?
        .into_iter()
        .map(|(terminal, count)| match count {
            CheckedCardinality::Exact(count) => Ok((terminal, count)),
            CheckedCardinality::ExceedsU128 => Err(CaseRankRunLoweringError::TerminalCountOverflow),
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if actual_counts != expected_counts {
        return Err(CaseRankRunLoweringError::TerminalCountMismatch {
            expected: expected_counts,
            actual: actual_counts,
        });
    }
    Ok(graph)
}

fn append_normalized_rank_segment(
    segments: &mut Vec<NormalizedCaseTerminalRankSegment>,
    start: u128,
    end_exclusive: u128,
    terminal: CaseTerminal,
    budget: &mut CaseRankRunLoweringBudget,
) -> Result<(), CaseRankRunLoweringError> {
    if start == end_exclusive {
        return Ok(());
    }
    if let Some(last) = segments.last_mut() {
        if last.end_exclusive != start {
            return Err(CaseRankRunLoweringError::InternalInvariant(
                "normalized rank segments are not contiguous",
            ));
        }
        if last.terminal == terminal {
            last.end_exclusive = end_exclusive;
            return Ok(());
        }
    } else if start != 0 {
        return Err(CaseRankRunLoweringError::InternalInvariant(
            "normalized rank segments do not begin at rank zero",
        ));
    }
    budget.charge_segment()?;
    segments
        .try_reserve(1)
        .map_err(|_| CaseRankRunLoweringError::AllocationFailed {
            resource: CaseRankRunLoweringResource::AccountedBytes,
        })?;
    segments.push(NormalizedCaseTerminalRankSegment {
        start,
        end_exclusive,
        terminal,
    });
    Ok(())
}

fn terminal_counts_from_rank_segments(
    segments: &[NormalizedCaseTerminalRankSegment],
    universe: u128,
) -> Result<BTreeMap<CaseTerminal, u128>, CaseRankRunLoweringError> {
    let mut counts = BTreeMap::new();
    let mut total = 0_u128;
    for segment in segments {
        let width = segment.end_exclusive.checked_sub(segment.start).ok_or(
            CaseRankRunLoweringError::InternalInvariant(
                "normalized rank segment is empty or reversed",
            ),
        )?;
        total = total
            .checked_add(width)
            .ok_or(CaseRankRunLoweringError::TerminalCountOverflow)?;
        let count = counts.entry(segment.terminal.clone()).or_insert(0_u128);
        *count = count
            .checked_add(width)
            .ok_or(CaseRankRunLoweringError::TerminalCountOverflow)?;
    }
    if total != universe {
        return Err(CaseRankRunLoweringError::InternalInvariant(
            "normalized rank terminal counts do not cover the universe",
        ));
    }
    Ok(counts)
}

fn validate_rank_run_graph_shape(
    graph: &CaseDecisionDag,
    limits: CaseRankRunLoweringLimits,
) -> Result<(), CaseRankRunLoweringError> {
    if graph.nodes.len() > limits.max_nodes() {
        return Err(CaseRankRunLoweringError::LimitExceeded {
            resource: CaseRankRunLoweringResource::Nodes,
            observed: graph.nodes.len(),
            limit: limits.max_nodes(),
        });
    }
    let mut arcs = 0_usize;
    let mut intervals = 0_usize;
    for node in &graph.nodes {
        arcs = arcs.checked_add(node.arcs.len()).ok_or(
            CaseRankRunLoweringError::ArithmeticOverflow("case DAG arc count"),
        )?;
        for arc in &node.arcs {
            intervals = intervals.checked_add(arc.ordinals.intervals.len()).ok_or(
                CaseRankRunLoweringError::ArithmeticOverflow("case DAG ordinal interval count"),
            )?;
        }
    }
    if arcs > limits.max_arcs() {
        return Err(CaseRankRunLoweringError::LimitExceeded {
            resource: CaseRankRunLoweringResource::Arcs,
            observed: arcs,
            limit: limits.max_arcs(),
        });
    }
    if intervals > limits.max_ordinal_intervals() {
        return Err(CaseRankRunLoweringError::LimitExceeded {
            resource: CaseRankRunLoweringResource::OrdinalIntervals,
            observed: intervals,
            limit: limits.max_ordinal_intervals(),
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct NodeId(usize);

impl NodeId {
    pub(super) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct TerminalId(usize);

impl TerminalId {
    pub(super) const fn index(self) -> usize {
        self.0
    }
}

/// A reference inside a decision DAG.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum DecisionRef {
    Node(NodeId),
    Terminal(TerminalId),
}

/// Empty domains have no decision target; all other products have one.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum DecisionRoot {
    EmptySpace,
    Target(DecisionRef),
}

/// One nonempty half-open set interval in domain-ordinal space.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct OrdinalInterval {
    start: DomainOrdinal,
    end_exclusive: DomainOrdinal,
}

impl OrdinalInterval {
    fn new(start: u128, end_exclusive: u128) -> Result<Self, CaseGraphError> {
        if start >= end_exclusive {
            return Err(CaseGraphError::InvalidGraph(format!(
                "ordinal interval [{start}, {end_exclusive}) is empty or reversed"
            )));
        }
        Ok(Self {
            start: DomainOrdinal(start),
            end_exclusive: DomainOrdinal(end_exclusive),
        })
    }

    pub(super) const fn start(self) -> DomainOrdinal {
        self.start
    }

    pub(super) const fn end_exclusive(self) -> DomainOrdinal {
        self.end_exclusive
    }

    pub(super) const fn len(self) -> u128 {
        self.end_exclusive.0 - self.start.0
    }

    pub(super) const fn contains(self, ordinal: DomainOrdinal) -> bool {
        self.start.0 <= ordinal.0 && ordinal.0 < self.end_exclusive.0
    }
}

/// A normalized, nonempty union of exact ordinal intervals.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct OrdinalSet {
    intervals: Vec<OrdinalInterval>,
}

impl OrdinalSet {
    fn from_normalized(intervals: Vec<OrdinalInterval>) -> Result<Self, CaseGraphError> {
        if intervals.is_empty() {
            return Err(CaseGraphError::InvalidGraph(
                "an ordinal set must contain at least one interval".to_string(),
            ));
        }
        for pair in intervals.windows(2) {
            if pair[0].end_exclusive.0 >= pair[1].start.0 {
                return Err(CaseGraphError::InvalidGraph(format!(
                    "ordinal intervals [{}, {}) and [{}, {}) overlap or were not coalesced",
                    pair[0].start.0,
                    pair[0].end_exclusive.0,
                    pair[1].start.0,
                    pair[1].end_exclusive.0
                )));
            }
        }
        Ok(Self { intervals })
    }

    pub(super) fn intervals(&self) -> &[OrdinalInterval] {
        &self.intervals
    }

    pub(super) fn contains(&self, ordinal: DomainOrdinal) -> bool {
        self.intervals
            .iter()
            .any(|interval| interval.contains(ordinal))
    }

    fn first_start(&self) -> u128 {
        self.intervals[0].start.0
    }

    fn checked_len(&self) -> Option<u128> {
        self.intervals
            .iter()
            .try_fold(0_u128, |total, interval| total.checked_add(interval.len()))
    }
}

/// One exhaustive branch of a decision node.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct DecisionArc {
    ordinals: OrdinalSet,
    child: DecisionRef,
}

impl DecisionArc {
    pub(super) fn ordinals(&self) -> &OrdinalSet {
        &self.ordinals
    }

    pub(super) const fn child(&self) -> DecisionRef {
        self.child
    }
}

/// One reduced decision on an independently varied axis.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct DecisionNode {
    dimension_index: usize,
    arcs: Vec<DecisionArc>,
}

impl DecisionNode {
    pub(super) const fn dimension_index(&self) -> usize {
        self.dimension_index
    }

    pub(super) fn arcs(&self) -> &[DecisionArc] {
        &self.arcs
    }
}

/// Checked evidence for a finite graph cardinality in the dependency-free
/// exact engine slice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum CheckedCardinality {
    Exact(u128),
    ExceedsU128,
}

impl CheckedCardinality {
    fn checked_add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact(left), Self::Exact(right)) => left
                .checked_add(right)
                .map(Self::Exact)
                .unwrap_or(Self::ExceedsU128),
            _ => Self::ExceedsU128,
        }
    }

    fn checked_mul(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact(0), _) | (_, Self::Exact(0)) => Self::Exact(0),
            (Self::Exact(left), Self::Exact(right)) => left
                .checked_mul(right)
                .map(Self::Exact)
                .unwrap_or(Self::ExceedsU128),
            _ => Self::ExceedsU128,
        }
    }
}

/// A canonical reduced ordered multi-terminal decision diagram.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OrderedDecisionDag<T> {
    axis_cardinalities: Vec<u128>,
    root: DecisionRoot,
    nodes: Vec<DecisionNode>,
    terminals: Vec<T>,
}

/// A checked, backend-neutral partition input for constructing an ordered
/// decision DAG without enumerating singleton paths.
///
/// The constructors normalize each local ordinal set. The final graph
/// constructor additionally checks source-order dimensions, bounds,
/// disjointness and exhaustiveness before any partition can become evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecisionPartition<T> {
    root: DecisionPartitionRoot<T>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecisionPartitionRoot<T> {
    EmptySpace,
    Target(DecisionPartitionTarget<T>),
}

impl<T> DecisionPartition<T> {
    pub(super) fn empty_space() -> Self {
        Self {
            root: DecisionPartitionRoot::EmptySpace,
        }
    }

    pub(super) fn target(target: DecisionPartitionTarget<T>) -> Self {
        Self {
            root: DecisionPartitionRoot::Target(target),
        }
    }
}

/// One terminal or source-order decision in a checked partition input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecisionPartitionTarget<T> {
    kind: DecisionPartitionTargetKind<T>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecisionPartitionTargetKind<T> {
    Terminal(T),
    Decision {
        dimension_index: usize,
        arcs: Vec<DecisionPartitionArc<T>>,
    },
}

impl<T> DecisionPartitionTarget<T> {
    pub(super) fn terminal(terminal: T) -> Self {
        Self {
            kind: DecisionPartitionTargetKind::Terminal(terminal),
        }
    }

    pub(super) fn decision(
        dimension_index: usize,
        arcs: Vec<DecisionPartitionArc<T>>,
    ) -> Result<Self, CaseGraphError> {
        if arcs.is_empty() {
            return Err(CaseGraphError::InvalidGraph(format!(
                "partition decision on dimension {dimension_index} has no arcs"
            )));
        }
        Ok(Self {
            kind: DecisionPartitionTargetKind::Decision {
                dimension_index,
                arcs,
            },
        })
    }
}

/// One exact ordinal-set branch in a checked partition input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecisionPartitionArc<T> {
    ordinals: OrdinalSet,
    child: Box<DecisionPartitionTarget<T>>,
}

impl<T> DecisionPartitionArc<T> {
    pub(super) fn new<I>(
        ordinal_intervals: I,
        child: DecisionPartitionTarget<T>,
    ) -> Result<Self, CaseGraphError>
    where
        I: IntoIterator<Item = (u128, u128)>,
    {
        let mut intervals = ordinal_intervals
            .into_iter()
            .map(|(start, end_exclusive)| OrdinalInterval::new(start, end_exclusive))
            .collect::<Result<Vec<_>, _>>()?;
        intervals.sort_by_key(|interval| (interval.start.0, interval.end_exclusive.0));

        let mut normalized: Vec<OrdinalInterval> = Vec::with_capacity(intervals.len());
        for interval in intervals {
            if let Some(last) = normalized.last_mut() {
                if interval.start.0 <= last.end_exclusive.0 {
                    last.end_exclusive.0 = last.end_exclusive.0.max(interval.end_exclusive.0);
                    continue;
                }
            }
            normalized.push(interval);
        }

        Ok(Self {
            ordinals: OrdinalSet::from_normalized(normalized)?,
            child: Box::new(child),
        })
    }
}

impl<T> OrderedDecisionDag<T> {
    pub(super) fn axis_cardinalities(&self) -> &[u128] {
        &self.axis_cardinalities
    }

    pub(super) const fn root(&self) -> DecisionRoot {
        self.root
    }

    pub(super) fn nodes(&self) -> &[DecisionNode] {
        &self.nodes
    }

    pub(super) fn terminals(&self) -> &[T] {
        &self.terminals
    }

    pub(super) fn node(&self, id: NodeId) -> Option<&DecisionNode> {
        self.nodes.get(id.0)
    }

    pub(super) fn terminal(&self, id: TerminalId) -> Option<&T> {
        self.terminals.get(id.0)
    }

    pub(super) fn universe_cardinality(&self) -> CheckedCardinality {
        checked_product(&self.axis_cardinalities)
    }

    /// Looks up one assignment without expanding dimensions omitted by
    /// reduction. `None` is returned only for the distinguished empty space.
    pub(super) fn terminal_for_path(
        &self,
        raw_path: &[u128],
    ) -> Result<Option<&T>, CaseGraphError> {
        if raw_path.len() != self.axis_cardinalities.len() {
            return Err(CaseGraphError::PathArity {
                expected: self.axis_cardinalities.len(),
                actual: raw_path.len(),
            });
        }
        self.validate_path(raw_path)?;
        let DecisionRoot::Target(mut target) = self.root else {
            return Ok(None);
        };

        loop {
            match target {
                DecisionRef::Terminal(id) => {
                    return self.terminal(id).map(Some).ok_or_else(|| {
                        CaseGraphError::InvalidGraph(format!(
                            "terminal reference {} is out of bounds",
                            id.0
                        ))
                    });
                }
                DecisionRef::Node(id) => {
                    let node = self.node(id).ok_or_else(|| {
                        CaseGraphError::InvalidGraph(format!(
                            "node reference {} is out of bounds",
                            id.0
                        ))
                    })?;
                    let ordinal = DomainOrdinal(raw_path[node.dimension_index]);
                    target = node
                        .arcs
                        .iter()
                        .find(|arc| arc.ordinals.contains(ordinal))
                        .map(|arc| arc.child)
                        .ok_or_else(|| {
                            CaseGraphError::InvalidGraph(format!(
                                "node {} does not classify ordinal {} of dimension {}",
                                id.0, ordinal.0, node.dimension_index
                            ))
                        })?;
                }
            }
        }
    }

    fn validate_path(&self, raw_path: &[u128]) -> Result<(), CaseGraphError> {
        if raw_path.len() != self.axis_cardinalities.len() {
            return Err(CaseGraphError::PathArity {
                expected: self.axis_cardinalities.len(),
                actual: raw_path.len(),
            });
        }
        for (dimension, (&ordinal, &cardinality)) in
            raw_path.iter().zip(&self.axis_cardinalities).enumerate()
        {
            if ordinal >= cardinality {
                return Err(CaseGraphError::OrdinalOutOfBounds {
                    dimension,
                    ordinal,
                    cardinality,
                });
            }
        }
        Ok(())
    }
}

impl<T: Clone + Ord> OrderedDecisionDag<T> {
    /// Builds a reduced decision DAG from sparse, arbitrarily ordered point
    /// classifications. Every unmentioned assignment resolves to
    /// `default_terminal`; the complement is represented by ordinal intervals
    /// and is never expanded into individual paths.
    ///
    /// Repeated copies of the same `(path, terminal)` classification are
    /// harmless. Giving one path two different terminals is an error.
    pub(super) fn from_sparse_classifications<I, P>(
        axis_cardinalities: Vec<u128>,
        classifications: I,
        default_terminal: T,
    ) -> Result<Self, CaseGraphError>
    where
        I: IntoIterator<Item = (P, T)>,
        P: AsRef<[u128]>,
    {
        let mut trie = SparseDecisionTrie::new();
        for (path, terminal) in classifications {
            let path = path.as_ref();
            validate_sparse_path(&axis_cardinalities, path)?;
            trie.insert(path, terminal)?;
        }

        if axis_cardinalities.contains(&0) {
            let graph = Self {
                axis_cardinalities,
                root: DecisionRoot::EmptySpace,
                nodes: Vec::new(),
                terminals: Vec::new(),
            };
            graph.validate()?;
            return Ok(graph);
        }

        let mut builder = SparseDecisionDagBuilder::new(axis_cardinalities, default_terminal);
        let root = builder.build_target(&trie, 0)?;
        let graph = Self {
            axis_cardinalities: builder.axis_cardinalities,
            root: DecisionRoot::Target(root),
            nodes: builder.nodes,
            terminals: builder.terminals,
        };
        graph.validate()?;
        Ok(graph)
    }

    /// Builds a reduced DAG from a complete interval partition without
    /// materializing any singleton case path.
    ///
    /// The input is accepted only when it is an exact total partition of the
    /// declared product: dimensions are source ordered, every local ordinal
    /// set is in bounds, and sibling arc sets are disjoint and exhaustive.
    pub(super) fn from_decision_partition(
        axis_cardinalities: Vec<u128>,
        partition: DecisionPartition<T>,
    ) -> Result<Self, CaseGraphError> {
        let has_empty_axis = axis_cardinalities.contains(&0);
        match partition.root {
            DecisionPartitionRoot::EmptySpace => {
                if !has_empty_axis {
                    return Err(CaseGraphError::InvalidGraph(
                        "partition uses empty_space for a nonempty product".to_string(),
                    ));
                }
                let graph = Self {
                    axis_cardinalities,
                    root: DecisionRoot::EmptySpace,
                    nodes: Vec::new(),
                    terminals: Vec::new(),
                };
                graph.validate()?;
                Ok(graph)
            }
            DecisionPartitionRoot::Target(target) => {
                if has_empty_axis {
                    return Err(CaseGraphError::InvalidGraph(
                        "partition targets a product containing an empty axis".to_string(),
                    ));
                }
                let mut builder = DecisionPartitionDagBuilder::new(axis_cardinalities);
                let root = builder.build_target(&target, 0)?;
                let graph = Self {
                    axis_cardinalities: builder.axis_cardinalities,
                    root: DecisionRoot::Target(root),
                    nodes: builder.nodes,
                    terminals: builder.terminals,
                };
                graph.validate()?;
                Ok(graph)
            }
        }
    }

    /// Canonically projects terminal meanings without enumerating assignments.
    /// Equal projected terminals and suffixes are re-interned, and decisions
    /// made redundant by the projection are eliminated. This is the narrow
    /// bridge used to compare an authoritative case population with another
    /// exact incidence DAG over the same axes.
    pub(super) fn project_terminals<U, F>(
        &self,
        mut project: F,
    ) -> Result<OrderedDecisionDag<U>, CaseGraphError>
    where
        U: Clone + Ord,
        F: FnMut(&T) -> U,
    {
        self.validate()?;
        let projected_terminals = self.terminals.iter().map(&mut project).collect::<Vec<_>>();
        let DecisionRoot::Target(source_root) = self.root else {
            let projected = OrderedDecisionDag {
                axis_cardinalities: self.axis_cardinalities.clone(),
                root: DecisionRoot::EmptySpace,
                nodes: Vec::new(),
                terminals: Vec::new(),
            };
            projected.validate()?;
            return Ok(projected);
        };

        let mut builder = TerminalProjectionDagBuilder::new(self, projected_terminals);
        let root = builder.project_target(source_root)?;
        let projected = OrderedDecisionDag {
            axis_cardinalities: self.axis_cardinalities.clone(),
            root: DecisionRoot::Target(root),
            nodes: builder.nodes,
            terminals: builder.terminals,
        };
        projected.validate()?;
        Ok(projected)
    }

    /// Counts paths by terminal with multiplicity for arc width and every
    /// skipped dimension. Memoization includes the incoming dimension context,
    /// because one shared suffix may be reached after different skips.
    pub(super) fn terminal_counts(
        &self,
    ) -> Result<BTreeMap<T, CheckedCardinality>, CaseGraphError> {
        let mut by_id = BTreeMap::new();
        let mut memo = BTreeMap::new();
        if let DecisionRoot::Target(target) = self.root {
            by_id = self.count_target(target, 0, &mut memo)?;
        }

        let mut counts = BTreeMap::new();
        for (terminal_id, count) in by_id {
            let terminal = self.terminal(terminal_id).ok_or_else(|| {
                CaseGraphError::InvalidGraph(format!(
                    "terminal reference {} is out of bounds",
                    terminal_id.0
                ))
            })?;
            counts.insert(terminal.clone(), count);
        }
        Ok(counts)
    }

    fn count_target(
        &self,
        target: DecisionRef,
        next_dimension: usize,
        memo: &mut BTreeMap<(DecisionRef, usize), BTreeMap<TerminalId, CheckedCardinality>>,
    ) -> Result<BTreeMap<TerminalId, CheckedCardinality>, CaseGraphError> {
        if let Some(counts) = memo.get(&(target, next_dimension)) {
            return Ok(counts.clone());
        }

        let counts = match target {
            DecisionRef::Terminal(terminal_id) => {
                if self.terminal(terminal_id).is_none() {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "terminal reference {} is out of bounds",
                        terminal_id.0
                    )));
                }
                BTreeMap::from([(
                    terminal_id,
                    checked_product(&self.axis_cardinalities[next_dimension..]),
                )])
            }
            DecisionRef::Node(node_id) => {
                let node = self.node(node_id).ok_or_else(|| {
                    CaseGraphError::InvalidGraph(format!(
                        "node reference {} is out of bounds",
                        node_id.0
                    ))
                })?;
                if node.dimension_index < next_dimension {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "node {} decides dimension {} after context already advanced to {}",
                        node_id.0, node.dimension_index, next_dimension
                    )));
                }

                let skipped =
                    checked_product(&self.axis_cardinalities[next_dimension..node.dimension_index]);
                let mut subtotal = BTreeMap::new();
                for arc in &node.arcs {
                    let arc_width = arc.ordinals.checked_len().ok_or_else(|| {
                        CaseGraphError::InvalidGraph(format!(
                            "ordinal width overflows u128 at node {}",
                            node_id.0
                        ))
                    })?;
                    let child_counts =
                        self.count_target(arc.child, node.dimension_index + 1, memo)?;
                    for (terminal_id, child_count) in child_counts {
                        let contribution =
                            CheckedCardinality::Exact(arc_width).checked_mul(child_count);
                        subtotal
                            .entry(terminal_id)
                            .and_modify(|count: &mut CheckedCardinality| {
                                *count = count.checked_add(contribution)
                            })
                            .or_insert(contribution);
                    }
                }
                for count in subtotal.values_mut() {
                    *count = skipped.checked_mul(*count);
                }
                subtotal
            }
        };

        memo.insert((target, next_dimension), counts.clone());
        Ok(counts)
    }

    /// Checks all structural reduction, coverage, reachability and exact-count
    /// conservation invariants carried by this representation.
    pub(super) fn validate(&self) -> Result<(), CaseGraphError> {
        let has_empty_axis = self.axis_cardinalities.contains(&0);
        match self.root {
            DecisionRoot::EmptySpace => {
                if !has_empty_axis {
                    return Err(CaseGraphError::InvalidGraph(
                        "empty_space requires at least one empty axis".to_string(),
                    ));
                }
                if !self.nodes.is_empty() || !self.terminals.is_empty() {
                    return Err(CaseGraphError::InvalidGraph(
                        "empty_space must not retain nodes or terminals".to_string(),
                    ));
                }
                return Ok(());
            }
            DecisionRoot::Target(_) if has_empty_axis => {
                return Err(CaseGraphError::InvalidGraph(
                    "a product with an empty axis must use empty_space".to_string(),
                ));
            }
            DecisionRoot::Target(DecisionRef::Node(_)) if self.axis_cardinalities.is_empty() => {
                return Err(CaseGraphError::InvalidGraph(
                    "the zero-axis singleton cannot contain a decision node".to_string(),
                ));
            }
            DecisionRoot::Target(_) => {}
        }

        let mut unique_terminals = BTreeSet::new();
        for terminal in &self.terminals {
            if !unique_terminals.insert(terminal) {
                return Err(CaseGraphError::InvalidGraph(
                    "equal terminals were not interned".to_string(),
                ));
            }
        }

        let mut unique_nodes = BTreeSet::new();
        for (node_index, node) in self.nodes.iter().enumerate() {
            if !unique_nodes.insert(node) {
                return Err(CaseGraphError::InvalidGraph(format!(
                    "node {node_index} duplicates an already interned node"
                )));
            }
            self.validate_node(NodeId(node_index), node)?;
        }

        let mut reachable_nodes = BTreeSet::new();
        let mut reachable_terminals = BTreeSet::new();
        if let DecisionRoot::Target(target) = self.root {
            self.collect_reachable(target, &mut reachable_nodes, &mut reachable_terminals)?;
        }
        if reachable_nodes.len() != self.nodes.len() {
            return Err(CaseGraphError::InvalidGraph(format!(
                "{} of {} decision nodes are reachable",
                reachable_nodes.len(),
                self.nodes.len()
            )));
        }
        if reachable_terminals.len() != self.terminals.len() {
            return Err(CaseGraphError::InvalidGraph(format!(
                "{} of {} terminals are reachable",
                reachable_terminals.len(),
                self.terminals.len()
            )));
        }

        let counts = self.terminal_counts()?;
        let counted = counts.values().copied().fold(
            CheckedCardinality::Exact(0),
            CheckedCardinality::checked_add,
        );
        if let (CheckedCardinality::Exact(counted), CheckedCardinality::Exact(expected)) =
            (counted, self.universe_cardinality())
        {
            if counted != expected {
                return Err(CaseGraphError::InvalidGraph(format!(
                    "terminal path counts total {counted}, expected universe cardinality {expected}"
                )));
            }
        }

        Ok(())
    }

    fn validate_node(&self, node_id: NodeId, node: &DecisionNode) -> Result<(), CaseGraphError> {
        let cardinality = self
            .axis_cardinalities
            .get(node.dimension_index)
            .copied()
            .ok_or_else(|| {
                CaseGraphError::InvalidGraph(format!(
                    "node {} has out-of-bounds dimension {}",
                    node_id.0, node.dimension_index
                ))
            })?;
        if cardinality == 0 {
            return Err(CaseGraphError::InvalidGraph(format!(
                "node {} decides an empty dimension",
                node_id.0
            )));
        }
        if node.arcs.len() < 2 {
            return Err(CaseGraphError::InvalidGraph(format!(
                "node {} was not eliminated even though its dimension has one child",
                node_id.0
            )));
        }

        let mut children = BTreeSet::new();
        let mut prior_arc_start = None;
        let mut flattened = Vec::new();
        for arc in &node.arcs {
            if arc.ordinals.intervals.is_empty() {
                return Err(CaseGraphError::InvalidGraph(format!(
                    "node {} contains an empty ordinal set",
                    node_id.0
                )));
            }
            if !children.insert(arc.child) {
                return Err(CaseGraphError::InvalidGraph(format!(
                    "node {} has multiple arcs to the same child",
                    node_id.0
                )));
            }
            let first_start = arc.ordinals.first_start();
            if prior_arc_start.is_some_and(|prior| prior >= first_start) {
                return Err(CaseGraphError::InvalidGraph(format!(
                    "node {} arcs are not in canonical first-ordinal order",
                    node_id.0
                )));
            }
            prior_arc_start = Some(first_start);

            for pair in arc.ordinals.intervals.windows(2) {
                if pair[0].end_exclusive.0 >= pair[1].start.0 {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "node {} contains overlapping, adjacent or unsorted intervals",
                        node_id.0
                    )));
                }
            }
            for interval in &arc.ordinals.intervals {
                if interval.start.0 >= interval.end_exclusive.0
                    || interval.end_exclusive.0 > cardinality
                {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "node {} has interval [{}, {}) outside dimension cardinality {}",
                        node_id.0, interval.start.0, interval.end_exclusive.0, cardinality
                    )));
                }
                flattened.push(*interval);
            }

            match arc.child {
                DecisionRef::Terminal(id) => {
                    if self.terminal(id).is_none() {
                        return Err(CaseGraphError::InvalidGraph(format!(
                            "node {} references missing terminal {}",
                            node_id.0, id.0
                        )));
                    }
                }
                DecisionRef::Node(id) => {
                    let child = self.node(id).ok_or_else(|| {
                        CaseGraphError::InvalidGraph(format!(
                            "node {} references missing node {}",
                            node_id.0, id.0
                        ))
                    })?;
                    if child.dimension_index <= node.dimension_index {
                        return Err(CaseGraphError::InvalidGraph(format!(
                            "node {} dimension {} does not precede child node {} dimension {}",
                            node_id.0, node.dimension_index, id.0, child.dimension_index
                        )));
                    }
                }
            }
        }

        flattened.sort_by_key(|interval| interval.start.0);
        let mut expected_start = 0_u128;
        for interval in flattened {
            if interval.start.0 != expected_start {
                return Err(CaseGraphError::InvalidGraph(format!(
                    "node {} has a gap or overlap at ordinal {}",
                    node_id.0, expected_start
                )));
            }
            expected_start = interval.end_exclusive.0;
        }
        if expected_start != cardinality {
            return Err(CaseGraphError::InvalidGraph(format!(
                "node {} covers ordinals through {}, expected {}",
                node_id.0, expected_start, cardinality
            )));
        }

        Ok(())
    }

    fn collect_reachable(
        &self,
        target: DecisionRef,
        nodes: &mut BTreeSet<NodeId>,
        terminals: &mut BTreeSet<TerminalId>,
    ) -> Result<(), CaseGraphError> {
        match target {
            DecisionRef::Terminal(id) => {
                if self.terminal(id).is_none() {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "terminal reference {} is out of bounds",
                        id.0
                    )));
                }
                terminals.insert(id);
            }
            DecisionRef::Node(id) => {
                let node = self.node(id).ok_or_else(|| {
                    CaseGraphError::InvalidGraph(format!(
                        "node reference {} is out of bounds",
                        id.0
                    ))
                })?;
                if !nodes.insert(id) {
                    return Ok(());
                }
                for arc in &node.arcs {
                    self.collect_reachable(arc.child, nodes, terminals)?;
                }
            }
        }
        Ok(())
    }
}

struct TerminalProjectionDagBuilder<'a, T, U> {
    source: &'a OrderedDecisionDag<T>,
    projected_terminals: Vec<U>,
    terminal_interner: BTreeMap<U, TerminalId>,
    terminals: Vec<U>,
    node_interner: BTreeMap<DecisionNode, NodeId>,
    nodes: Vec<DecisionNode>,
    memo: BTreeMap<DecisionRef, DecisionRef>,
}

impl<'a, T, U: Clone + Ord> TerminalProjectionDagBuilder<'a, T, U> {
    fn new(source: &'a OrderedDecisionDag<T>, projected_terminals: Vec<U>) -> Self {
        Self {
            source,
            projected_terminals,
            terminal_interner: BTreeMap::new(),
            terminals: Vec::new(),
            node_interner: BTreeMap::new(),
            nodes: Vec::new(),
            memo: BTreeMap::new(),
        }
    }

    fn project_target(
        &mut self,
        source_target: DecisionRef,
    ) -> Result<DecisionRef, CaseGraphError> {
        if let Some(projected) = self.memo.get(&source_target) {
            return Ok(*projected);
        }

        let projected = match source_target {
            DecisionRef::Terminal(source_id) => {
                let terminal = self
                    .projected_terminals
                    .get(source_id.0)
                    .ok_or_else(|| {
                        CaseGraphError::InvalidGraph(format!(
                            "terminal projection references missing source terminal {}",
                            source_id.0
                        ))
                    })?
                    .clone();
                DecisionRef::Terminal(self.intern_terminal(terminal))
            }
            DecisionRef::Node(source_id) => {
                let source_node = self.source.node(source_id).ok_or_else(|| {
                    CaseGraphError::InvalidGraph(format!(
                        "terminal projection references missing source node {}",
                        source_id.0
                    ))
                })?;
                let dimension_index = source_node.dimension_index;
                let source_arcs = source_node.arcs.clone();
                let mut intervals_by_child = BTreeMap::<DecisionRef, Vec<OrdinalInterval>>::new();
                for source_arc in source_arcs {
                    let child = self.project_target(source_arc.child)?;
                    intervals_by_child
                        .entry(child)
                        .or_default()
                        .extend(source_arc.ordinals.intervals);
                }

                if intervals_by_child.len() == 1 {
                    intervals_by_child.into_keys().next().ok_or(
                        CaseGraphError::InternalInvariant(
                            "a source decision unexpectedly projected to no children",
                        ),
                    )?
                } else {
                    let mut arcs = intervals_by_child
                        .into_iter()
                        .map(|(child, mut intervals)| {
                            intervals.sort_by_key(|interval| {
                                (interval.start.0, interval.end_exclusive.0)
                            });
                            let mut normalized: Vec<OrdinalInterval> =
                                Vec::with_capacity(intervals.len());
                            for interval in intervals {
                                if let Some(last) = normalized.last_mut() {
                                    if interval.start.0 <= last.end_exclusive.0 {
                                        last.end_exclusive.0 =
                                            last.end_exclusive.0.max(interval.end_exclusive.0);
                                        continue;
                                    }
                                }
                                normalized.push(interval);
                            }
                            Ok(DecisionArc {
                                ordinals: OrdinalSet::from_normalized(normalized)?,
                                child,
                            })
                        })
                        .collect::<Result<Vec<_>, CaseGraphError>>()?;
                    arcs.sort_by_key(|arc| arc.ordinals.first_start());
                    DecisionRef::Node(self.intern_node(DecisionNode {
                        dimension_index,
                        arcs,
                    }))
                }
            }
        };
        self.memo.insert(source_target, projected);
        Ok(projected)
    }

    fn intern_terminal(&mut self, terminal: U) -> TerminalId {
        if let Some(id) = self.terminal_interner.get(&terminal) {
            return *id;
        }
        let id = TerminalId(self.terminals.len());
        self.terminals.push(terminal.clone());
        self.terminal_interner.insert(terminal, id);
        id
    }

    fn intern_node(&mut self, node: DecisionNode) -> NodeId {
        if let Some(id) = self.node_interner.get(&node) {
            return *id;
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(node.clone());
        self.node_interner.insert(node, id);
        id
    }
}

struct SparseDecisionTrie<T> {
    terminal: Option<T>,
    children: BTreeMap<u128, SparseDecisionTrie<T>>,
}

impl<T: Ord> SparseDecisionTrie<T> {
    fn new() -> Self {
        Self {
            terminal: None,
            children: BTreeMap::new(),
        }
    }

    fn insert(&mut self, path: &[u128], terminal: T) -> Result<(), CaseGraphError> {
        let mut trie = self;
        for &ordinal in path {
            trie = trie
                .children
                .entry(ordinal)
                .or_insert_with(SparseDecisionTrie::new);
        }
        match &trie.terminal {
            Some(existing) if existing != &terminal => Err(CaseGraphError::DuplicatePathConflict {
                path: path.to_vec(),
            }),
            Some(_) => Ok(()),
            None => {
                trie.terminal = Some(terminal);
                Ok(())
            }
        }
    }
}

struct SparseDecisionDagBuilder<T> {
    axis_cardinalities: Vec<u128>,
    default_terminal: T,
    terminal_interner: BTreeMap<T, TerminalId>,
    terminals: Vec<T>,
    node_interner: BTreeMap<DecisionNode, NodeId>,
    nodes: Vec<DecisionNode>,
}

impl<T: Clone + Ord> SparseDecisionDagBuilder<T> {
    fn new(axis_cardinalities: Vec<u128>, default_terminal: T) -> Self {
        Self {
            axis_cardinalities,
            default_terminal,
            terminal_interner: BTreeMap::new(),
            terminals: Vec::new(),
            node_interner: BTreeMap::new(),
            nodes: Vec::new(),
        }
    }

    fn build_target(
        &mut self,
        trie: &SparseDecisionTrie<T>,
        dimension: usize,
    ) -> Result<DecisionRef, CaseGraphError> {
        if dimension == self.axis_cardinalities.len() {
            if !trie.children.is_empty() {
                return Err(CaseGraphError::InternalInvariant(
                    "a sparse trie leaf retained children beyond the final dimension",
                ));
            }
            let terminal = trie
                .terminal
                .as_ref()
                .unwrap_or(&self.default_terminal)
                .clone();
            return Ok(DecisionRef::Terminal(self.intern_terminal(terminal)));
        }
        if trie.terminal.is_some() {
            return Err(CaseGraphError::InternalInvariant(
                "a sparse trie classified a path before the final dimension",
            ));
        }
        if trie.children.is_empty() {
            return Ok(self.default_target());
        }

        let cardinality = self.axis_cardinalities[dimension];
        let mut runs = Vec::new();
        let mut next_ordinal = 0_u128;
        for (&ordinal, child_trie) in &trie.children {
            if next_ordinal < ordinal {
                let default = self.default_target();
                append_sparse_run(&mut runs, next_ordinal, ordinal, default);
            }
            let child = self.build_target(child_trie, dimension + 1)?;
            let end_exclusive = ordinal
                .checked_add(1)
                .ok_or(CaseGraphError::InternalInvariant(
                    "a validated sparse ordinal overflowed",
                ))?;
            append_sparse_run(&mut runs, ordinal, end_exclusive, child);
            next_ordinal = end_exclusive;
        }
        if next_ordinal < cardinality {
            let default = self.default_target();
            append_sparse_run(&mut runs, next_ordinal, cardinality, default);
        }

        if runs.len() == 1 {
            return Ok(runs[0].child);
        }

        let mut intervals_by_child: BTreeMap<DecisionRef, Vec<OrdinalInterval>> = BTreeMap::new();
        for run in runs {
            intervals_by_child
                .entry(run.child)
                .or_default()
                .push(OrdinalInterval::new(run.start, run.end_exclusive)?);
        }
        let mut arcs = intervals_by_child
            .into_iter()
            .map(|(child, intervals)| {
                Ok(DecisionArc {
                    ordinals: OrdinalSet::from_normalized(intervals)?,
                    child,
                })
            })
            .collect::<Result<Vec<_>, CaseGraphError>>()?;
        arcs.sort_by_key(|arc| arc.ordinals.first_start());

        let node = DecisionNode {
            dimension_index: dimension,
            arcs,
        };
        Ok(DecisionRef::Node(self.intern_node(node)))
    }

    fn default_target(&mut self) -> DecisionRef {
        DecisionRef::Terminal(self.intern_terminal(self.default_terminal.clone()))
    }

    fn intern_terminal(&mut self, terminal: T) -> TerminalId {
        if let Some(id) = self.terminal_interner.get(&terminal) {
            return *id;
        }
        let id = TerminalId(self.terminals.len());
        self.terminals.push(terminal.clone());
        self.terminal_interner.insert(terminal, id);
        id
    }

    fn intern_node(&mut self, node: DecisionNode) -> NodeId {
        if let Some(id) = self.node_interner.get(&node) {
            return *id;
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(node.clone());
        self.node_interner.insert(node, id);
        id
    }
}

/// Converts a validated total interval partition into the graph's private
/// interned representation. Equal suffixes are interned after recursive
/// lowering, so input trees become canonical DAGs.
struct DecisionPartitionDagBuilder<T> {
    axis_cardinalities: Vec<u128>,
    terminal_interner: BTreeMap<T, TerminalId>,
    terminals: Vec<T>,
    node_interner: BTreeMap<DecisionNode, NodeId>,
    nodes: Vec<DecisionNode>,
}

impl<T: Clone + Ord> DecisionPartitionDagBuilder<T> {
    fn new(axis_cardinalities: Vec<u128>) -> Self {
        Self {
            axis_cardinalities,
            terminal_interner: BTreeMap::new(),
            terminals: Vec::new(),
            node_interner: BTreeMap::new(),
            nodes: Vec::new(),
        }
    }

    fn build_target(
        &mut self,
        target: &DecisionPartitionTarget<T>,
        next_dimension: usize,
    ) -> Result<DecisionRef, CaseGraphError> {
        match &target.kind {
            DecisionPartitionTargetKind::Terminal(terminal) => Ok(DecisionRef::Terminal(
                self.intern_terminal(terminal.clone()),
            )),
            DecisionPartitionTargetKind::Decision {
                dimension_index,
                arcs,
            } => {
                if *dimension_index < next_dimension {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "partition dimension {dimension_index} does not follow source-order context {next_dimension}"
                    )));
                }
                let cardinality = self
                    .axis_cardinalities
                    .get(*dimension_index)
                    .copied()
                    .ok_or_else(|| {
                        CaseGraphError::InvalidGraph(format!(
                            "partition dimension {dimension_index} is outside {} declared axes",
                            self.axis_cardinalities.len()
                        ))
                    })?;
                if cardinality == 0 {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "partition decides empty dimension {dimension_index}"
                    )));
                }

                let mut flattened = Vec::new();
                for arc in arcs {
                    let child = self.build_target(&arc.child, *dimension_index + 1)?;
                    for interval in arc.ordinals.intervals() {
                        if interval.end_exclusive.0 > cardinality {
                            return Err(CaseGraphError::InvalidGraph(format!(
                                "partition interval [{}, {}) is outside dimension {dimension_index} cardinality {cardinality}",
                                interval.start.0, interval.end_exclusive.0
                            )));
                        }
                        flattened.push((*interval, child));
                    }
                }
                flattened.sort_by_key(|(interval, child)| {
                    (interval.start.0, interval.end_exclusive.0, *child)
                });

                let mut expected_start = 0_u128;
                for (interval, _) in &flattened {
                    if interval.start.0 < expected_start {
                        return Err(CaseGraphError::InvalidGraph(format!(
                            "partition arcs overlap on dimension {dimension_index} at ordinal {}",
                            interval.start.0
                        )));
                    }
                    if interval.start.0 > expected_start {
                        return Err(CaseGraphError::InvalidGraph(format!(
                            "partition arcs leave a gap on dimension {dimension_index} at ordinal {expected_start}"
                        )));
                    }
                    expected_start = interval.end_exclusive.0;
                }
                if expected_start != cardinality {
                    return Err(CaseGraphError::InvalidGraph(format!(
                        "partition arcs cover dimension {dimension_index} through {expected_start}, expected {cardinality}"
                    )));
                }

                let mut intervals_by_child: BTreeMap<DecisionRef, Vec<OrdinalInterval>> =
                    BTreeMap::new();
                for (interval, child) in flattened {
                    let child_intervals = intervals_by_child.entry(child).or_default();
                    if let Some(last) = child_intervals.last_mut() {
                        if last.end_exclusive.0 == interval.start.0 {
                            last.end_exclusive = interval.end_exclusive;
                            continue;
                        }
                    }
                    child_intervals.push(interval);
                }

                if intervals_by_child.len() == 1 {
                    return Ok(*intervals_by_child
                        .first_key_value()
                        .expect("one partition child was just checked")
                        .0);
                }

                let mut normalized_arcs = intervals_by_child
                    .into_iter()
                    .map(|(child, intervals)| {
                        Ok(DecisionArc {
                            ordinals: OrdinalSet::from_normalized(intervals)?,
                            child,
                        })
                    })
                    .collect::<Result<Vec<_>, CaseGraphError>>()?;
                normalized_arcs.sort_by_key(|arc| arc.ordinals.first_start());
                let node = DecisionNode {
                    dimension_index: *dimension_index,
                    arcs: normalized_arcs,
                };
                Ok(DecisionRef::Node(self.intern_node(node)))
            }
        }
    }

    fn intern_terminal(&mut self, terminal: T) -> TerminalId {
        if let Some(id) = self.terminal_interner.get(&terminal) {
            return *id;
        }
        let id = TerminalId(self.terminals.len());
        self.terminals.push(terminal.clone());
        self.terminal_interner.insert(terminal, id);
        id
    }

    fn intern_node(&mut self, node: DecisionNode) -> NodeId {
        if let Some(id) = self.node_interner.get(&node) {
            return *id;
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(node.clone());
        self.node_interner.insert(node, id);
        id
    }
}

impl DecisionPartitionDagBuilder<CaseTerminal> {
    #[allow(clippy::too_many_arguments)]
    fn build_rank_segment_target(
        &mut self,
        segments: &[NormalizedCaseTerminalRankSegment],
        segment_start: usize,
        segment_end: usize,
        suffix_strides: &[u128],
        dimension: usize,
        base: u128,
        budget: &mut CaseRankRunLoweringBudget,
    ) -> Result<DecisionRef, CaseRankRunLoweringError> {
        if segment_start >= segment_end || segment_end > segments.len() {
            return Err(CaseRankRunLoweringError::InternalInvariant(
                "rank block received an empty or out-of-bounds segment slice",
            ));
        }
        let block_span = suffix_strides.get(dimension).copied().ok_or(
            CaseRankRunLoweringError::InternalInvariant(
                "rank block dimension has no suffix stride",
            ),
        )?;
        let block_end =
            base.checked_add(block_span)
                .ok_or(CaseRankRunLoweringError::ArithmeticOverflow(
                    "rank block endpoint",
                ))?;
        let first = &segments[segment_start];
        if first.start <= base && first.end_exclusive >= block_end {
            return self.intern_rank_terminal(first.terminal.clone(), budget);
        }
        if dimension >= self.axis_cardinalities.len() {
            return Err(CaseRankRunLoweringError::InternalInvariant(
                "a singleton rank block contains a terminal boundary",
            ));
        }

        let cardinality = self.axis_cardinalities[dimension];
        let child_span = suffix_strides[dimension + 1];
        if cardinality == 0 || child_span == 0 {
            return Err(CaseRankRunLoweringError::InternalInvariant(
                "nonempty rank lowering reached an empty mixed-radix axis",
            ));
        }

        let segment_count = segment_end.checked_sub(segment_start).ok_or(
            CaseRankRunLoweringError::InternalInvariant("rank segment range is reversed"),
        )?;
        let cut_capacity = segment_count
            .checked_mul(2)
            .and_then(|count| count.checked_add(2))
            .ok_or(CaseRankRunLoweringError::ArithmeticOverflow(
                "rank-boundary cut capacity",
            ))?;
        budget.charge_cuts(cut_capacity)?;
        let mut cuts = Vec::new();
        cuts.try_reserve_exact(cut_capacity).map_err(|_| {
            CaseRankRunLoweringError::AllocationFailed {
                resource: CaseRankRunLoweringResource::AccountedBytes,
            }
        })?;
        cuts.push(0_u128);
        cuts.push(cardinality);
        for segment in &segments[segment_start..segment_end] {
            let boundary = segment.end_exclusive;
            if boundary <= base || boundary >= block_end {
                continue;
            }
            let offset =
                boundary
                    .checked_sub(base)
                    .ok_or(CaseRankRunLoweringError::InternalInvariant(
                        "rank boundary precedes its enclosing block",
                    ))?;
            let ordinal = offset / child_span;
            let remainder = offset % child_span;
            if ordinal > cardinality {
                return Err(CaseRankRunLoweringError::InternalInvariant(
                    "rank boundary projects outside its mixed-radix axis",
                ));
            }
            cuts.push(ordinal);
            if remainder != 0 {
                let after =
                    ordinal
                        .checked_add(1)
                        .ok_or(CaseRankRunLoweringError::ArithmeticOverflow(
                            "boundary-pierced child ordinal",
                        ))?;
                if after > cardinality {
                    return Err(CaseRankRunLoweringError::InternalInvariant(
                        "boundary-pierced child exceeds its axis cardinality",
                    ));
                }
                cuts.push(after);
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        if cuts.first() != Some(&0) || cuts.last() != Some(&cardinality) {
            return Err(CaseRankRunLoweringError::InternalInvariant(
                "rank-boundary cuts do not cover the current axis",
            ));
        }

        let mut intervals_by_child = BTreeMap::<DecisionRef, Vec<OrdinalInterval>>::new();
        let mut scan = segment_start;
        for pair in cuts.windows(2) {
            let ordinal_start = pair[0];
            let ordinal_end = pair[1];
            if ordinal_start >= ordinal_end {
                return Err(CaseRankRunLoweringError::InternalInvariant(
                    "rank-boundary cuts contain an empty ordinal band",
                ));
            }
            let rank_start = ordinal_start
                .checked_mul(child_span)
                .and_then(|offset| base.checked_add(offset))
                .ok_or(CaseRankRunLoweringError::ArithmeticOverflow(
                    "ordinal-band start rank",
                ))?;
            let rank_end = ordinal_end
                .checked_mul(child_span)
                .and_then(|offset| base.checked_add(offset))
                .ok_or(CaseRankRunLoweringError::ArithmeticOverflow(
                    "ordinal-band end rank",
                ))?;
            while scan < segment_end && segments[scan].end_exclusive <= rank_start {
                scan += 1;
            }
            if scan >= segment_end || segments[scan].start > rank_start {
                return Err(CaseRankRunLoweringError::InternalInvariant(
                    "ordinal band is not covered by normalized rank segments",
                ));
            }
            let child_segment_start = scan;
            let mut child_segment_end = child_segment_start;
            while child_segment_end < segment_end && segments[child_segment_end].start < rank_end {
                child_segment_end += 1;
            }
            if child_segment_end == child_segment_start {
                return Err(CaseRankRunLoweringError::InternalInvariant(
                    "ordinal band intersects no normalized rank segment",
                ));
            }

            let child = if ordinal_end - ordinal_start == 1 {
                self.build_rank_segment_target(
                    segments,
                    child_segment_start,
                    child_segment_end,
                    suffix_strides,
                    dimension + 1,
                    rank_start,
                    budget,
                )?
            } else {
                let segment = &segments[child_segment_start];
                if child_segment_end != child_segment_start + 1
                    || segment.start > rank_start
                    || segment.end_exclusive < rank_end
                {
                    return Err(CaseRankRunLoweringError::InternalInvariant(
                        "a wide ordinal band contains an unprojected rank boundary",
                    ));
                }
                self.intern_rank_terminal(segment.terminal.clone(), budget)?
            };
            let interval = OrdinalInterval::new(ordinal_start, ordinal_end)?;
            if let Some(last) = intervals_by_child
                .get_mut(&child)
                .and_then(|intervals| intervals.last_mut())
            {
                if last.end_exclusive == interval.start {
                    last.end_exclusive = interval.end_exclusive;
                    continue;
                }
            }
            budget.charge_ordinal_interval_construction()?;
            let child_intervals = intervals_by_child.entry(child).or_default();
            child_intervals.try_reserve(1).map_err(|_| {
                CaseRankRunLoweringError::AllocationFailed {
                    resource: CaseRankRunLoweringResource::OrdinalIntervals,
                }
            })?;
            child_intervals.push(interval);
        }

        if intervals_by_child.len() == 1 {
            return Ok(*intervals_by_child
                .first_key_value()
                .expect("one rank-lowered child was just checked")
                .0);
        }
        budget.charge_node_construction()?;
        budget.charge_arc_construction(intervals_by_child.len())?;
        let mut arcs = Vec::new();
        arcs.try_reserve_exact(intervals_by_child.len())
            .map_err(|_| CaseRankRunLoweringError::AllocationFailed {
                resource: CaseRankRunLoweringResource::Arcs,
            })?;
        for (child, intervals) in intervals_by_child {
            arcs.push(DecisionArc {
                ordinals: OrdinalSet::from_normalized(intervals)?,
                child,
            });
        }
        arcs.sort_by_key(|arc| arc.ordinals.first_start());
        self.intern_rank_node(
            DecisionNode {
                dimension_index: dimension,
                arcs,
            },
            budget,
        )
    }

    fn intern_rank_terminal(
        &mut self,
        terminal: CaseTerminal,
        budget: &mut CaseRankRunLoweringBudget,
    ) -> Result<DecisionRef, CaseRankRunLoweringError> {
        if let Some(id) = self.terminal_interner.get(&terminal) {
            return Ok(DecisionRef::Terminal(*id));
        }
        budget.charge_terminal()?;
        self.terminals
            .try_reserve(1)
            .map_err(|_| CaseRankRunLoweringError::AllocationFailed {
                resource: CaseRankRunLoweringResource::AccountedBytes,
            })?;
        let id = TerminalId(self.terminals.len());
        self.terminals.push(terminal.clone());
        self.terminal_interner.insert(terminal, id);
        Ok(DecisionRef::Terminal(id))
    }

    fn intern_rank_node(
        &mut self,
        node: DecisionNode,
        budget: &mut CaseRankRunLoweringBudget,
    ) -> Result<DecisionRef, CaseRankRunLoweringError> {
        if let Some(id) = self.node_interner.get(&node) {
            return Ok(DecisionRef::Node(*id));
        }
        let interval_count = node.arcs.iter().try_fold(0_usize, |count, arc| {
            count.checked_add(arc.ordinals.intervals.len()).ok_or(
                CaseRankRunLoweringError::ArithmeticOverflow(
                    "unique case DAG ordinal interval count",
                ),
            )
        })?;
        budget.charge_unique_node()?;
        budget.charge_unique_arcs(node.arcs.len())?;
        budget.charge_unique_ordinal_intervals(interval_count)?;
        self.nodes
            .try_reserve(1)
            .map_err(|_| CaseRankRunLoweringError::AllocationFailed {
                resource: CaseRankRunLoweringResource::Nodes,
            })?;
        let id = NodeId(self.nodes.len());
        self.nodes.push(node.clone());
        self.node_interner.insert(node, id);
        Ok(DecisionRef::Node(id))
    }
}

fn append_sparse_run(
    runs: &mut Vec<OrdinalRun>,
    start: u128,
    end_exclusive: u128,
    child: DecisionRef,
) {
    if start == end_exclusive {
        return;
    }
    if let Some(last) = runs.last_mut() {
        if last.child == child && last.end_exclusive == start {
            last.end_exclusive = end_exclusive;
            return;
        }
    }
    runs.push(OrdinalRun {
        start,
        end_exclusive,
        child,
    });
}

fn validate_sparse_path(
    axis_cardinalities: &[u128],
    raw_path: &[u128],
) -> Result<(), CaseGraphError> {
    if raw_path.len() != axis_cardinalities.len() {
        return Err(CaseGraphError::PathArity {
            expected: axis_cardinalities.len(),
            actual: raw_path.len(),
        });
    }
    for (dimension, (&ordinal, &cardinality)) in raw_path.iter().zip(axis_cardinalities).enumerate()
    {
        if ordinal >= cardinality {
            return Err(CaseGraphError::OrdinalOutOfBounds {
                dimension,
                ordinal,
                cardinality,
            });
        }
    }
    Ok(())
}

/// A streaming bottom-up builder. Inputs must arrive at `next_path()`; this
/// makes an untouched suffix representable without storing a sparse trie.
pub(super) struct OrderedDecisionDagBuilder<T> {
    axis_cardinalities: Vec<u128>,
    next_path: Option<CaseOrdinalPath>,
    frames: Vec<DimensionFrame>,
    completed_root: Option<DecisionRef>,
    terminal_interner: BTreeMap<T, TerminalId>,
    terminals: Vec<T>,
    node_interner: BTreeMap<DecisionNode, NodeId>,
    nodes: Vec<DecisionNode>,
}

impl<T: Clone + Ord> OrderedDecisionDagBuilder<T> {
    pub(super) fn new(axis_cardinalities: Vec<u128>) -> Self {
        let has_empty_axis = axis_cardinalities.contains(&0);
        let next_path = if has_empty_axis {
            None
        } else {
            Some(CaseOrdinalPath(
                axis_cardinalities
                    .iter()
                    .map(|_| DomainOrdinal(0))
                    .collect(),
            ))
        };
        let frames = axis_cardinalities
            .iter()
            .copied()
            .map(DimensionFrame::new)
            .collect();
        Self {
            axis_cardinalities,
            next_path,
            frames,
            completed_root: None,
            terminal_interner: BTreeMap::new(),
            terminals: Vec::new(),
            node_interner: BTreeMap::new(),
            nodes: Vec::new(),
        }
    }

    pub(super) fn next_path(&self) -> Option<&CaseOrdinalPath> {
        self.next_path.as_ref()
    }

    /// Classifies exactly the next canonical path.
    pub(super) fn classify(
        &mut self,
        raw_path: &[u128],
        terminal: T,
    ) -> Result<(), CaseGraphError> {
        let expected = self
            .next_path
            .as_ref()
            .ok_or(CaseGraphError::NoRemainingPath)?;
        if !expected.matches_raw(raw_path) {
            return Err(CaseGraphError::UnexpectedPath {
                expected: expected.to_raw_ordinals(),
                actual: raw_path.to_vec(),
            });
        }
        self.push_next(terminal)
    }

    /// Classifies the current `next_path()` and advances in mixed-radix order.
    pub(super) fn push_next(&mut self, terminal: T) -> Result<(), CaseGraphError> {
        if self.next_path.is_none() {
            return Err(CaseGraphError::NoRemainingPath);
        }
        let terminal_ref = DecisionRef::Terminal(self.intern_terminal(terminal));

        if self.axis_cardinalities.is_empty() {
            if self.completed_root.replace(terminal_ref).is_some() {
                return Err(CaseGraphError::InternalInvariant(
                    "zero-axis root was completed more than once",
                ));
            }
        } else {
            self.push_resolved_suffix(terminal_ref)?;
        }
        self.advance_path()?;
        Ok(())
    }

    /// Finishes only after every nonempty-domain path was explicitly supplied.
    pub(super) fn finish_complete(self) -> Result<OrderedDecisionDag<T>, CaseGraphError> {
        if self.axis_cardinalities.contains(&0) {
            return self.finish_empty_space();
        }
        if let Some(next) = &self.next_path {
            return Err(CaseGraphError::IncompleteClassification {
                next: next.to_raw_ordinals(),
            });
        }
        self.finish_target()
    }

    /// Finishes a canonical classified prefix and maps its untouched suffix to
    /// `remainder`. No case in that suffix is enumerated.
    pub(super) fn finish_with_remainder(
        mut self,
        remainder: T,
    ) -> Result<OrderedDecisionDag<T>, CaseGraphError> {
        if self.axis_cardinalities.contains(&0) {
            return self.finish_empty_space();
        }
        if self.next_path.is_none() {
            return self.finish_target();
        }

        let remainder_ref = DecisionRef::Terminal(self.intern_terminal(remainder));
        if self.axis_cardinalities.is_empty() {
            self.completed_root = Some(remainder_ref);
            self.next_path = None;
            return self.finish_target();
        }

        let last_dimension = self.axis_cardinalities.len() - 1;
        self.frames[last_dimension].fill_remaining(remainder_ref)?;
        let mut suffix = self.resolve_complete_frame(last_dimension)?;

        for dimension in (0..last_dimension).rev() {
            self.frames[dimension].push_one(suffix)?;
            self.frames[dimension].fill_remaining(remainder_ref)?;
            suffix = self.resolve_complete_frame(dimension)?;
        }
        self.completed_root = Some(suffix);
        self.next_path = None;
        self.finish_target()
    }

    fn finish_empty_space(self) -> Result<OrderedDecisionDag<T>, CaseGraphError> {
        let graph = OrderedDecisionDag {
            axis_cardinalities: self.axis_cardinalities,
            root: DecisionRoot::EmptySpace,
            nodes: Vec::new(),
            terminals: Vec::new(),
        };
        graph.validate()?;
        Ok(graph)
    }

    fn finish_target(self) -> Result<OrderedDecisionDag<T>, CaseGraphError> {
        let root = self
            .completed_root
            .ok_or(CaseGraphError::InternalInvariant(
                "a nonempty case space finished without a root",
            ))?;
        let graph = OrderedDecisionDag {
            axis_cardinalities: self.axis_cardinalities,
            root: DecisionRoot::Target(root),
            nodes: self.nodes,
            terminals: self.terminals,
        };
        graph.validate()?;
        Ok(graph)
    }

    fn push_resolved_suffix(&mut self, mut suffix: DecisionRef) -> Result<(), CaseGraphError> {
        let mut dimension = self.axis_cardinalities.len() - 1;
        loop {
            self.frames[dimension].push_one(suffix)?;
            if !self.frames[dimension].is_complete() {
                return Ok(());
            }
            suffix = self.resolve_complete_frame(dimension)?;
            if dimension == 0 {
                if self.completed_root.replace(suffix).is_some() {
                    return Err(CaseGraphError::InternalInvariant(
                        "decision root was completed more than once",
                    ));
                }
                return Ok(());
            }
            dimension -= 1;
        }
    }

    fn resolve_complete_frame(&mut self, dimension: usize) -> Result<DecisionRef, CaseGraphError> {
        let runs = self.frames[dimension].take_complete()?;
        if runs.len() == 1 {
            return Ok(runs[0].child);
        }

        let mut intervals_by_child: BTreeMap<DecisionRef, Vec<OrdinalInterval>> = BTreeMap::new();
        for run in runs {
            intervals_by_child
                .entry(run.child)
                .or_default()
                .push(OrdinalInterval::new(run.start, run.end_exclusive)?);
        }
        let mut arcs = intervals_by_child
            .into_iter()
            .map(|(child, intervals)| {
                Ok(DecisionArc {
                    ordinals: OrdinalSet::from_normalized(intervals)?,
                    child,
                })
            })
            .collect::<Result<Vec<_>, CaseGraphError>>()?;
        arcs.sort_by_key(|arc| arc.ordinals.first_start());

        let node = DecisionNode {
            dimension_index: dimension,
            arcs,
        };
        Ok(DecisionRef::Node(self.intern_node(node)))
    }

    fn intern_terminal(&mut self, terminal: T) -> TerminalId {
        if let Some(id) = self.terminal_interner.get(&terminal) {
            return *id;
        }
        let id = TerminalId(self.terminals.len());
        self.terminals.push(terminal.clone());
        self.terminal_interner.insert(terminal, id);
        id
    }

    fn intern_node(&mut self, node: DecisionNode) -> NodeId {
        if let Some(id) = self.node_interner.get(&node) {
            return *id;
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(node.clone());
        self.node_interner.insert(node, id);
        id
    }

    fn advance_path(&mut self) -> Result<(), CaseGraphError> {
        let path = self
            .next_path
            .as_mut()
            .ok_or(CaseGraphError::InternalInvariant(
                "attempted to advance a completed path stream",
            ))?;
        if path.0.is_empty() {
            self.next_path = None;
            return Ok(());
        }

        for dimension in (0..path.0.len()).rev() {
            let next =
                path.0[dimension]
                    .0
                    .checked_add(1)
                    .ok_or(CaseGraphError::InternalInvariant(
                        "domain ordinal overflowed before its cardinality",
                    ))?;
            if next < self.axis_cardinalities[dimension] {
                path.0[dimension] = DomainOrdinal(next);
                for trailing in &mut path.0[dimension + 1..] {
                    *trailing = DomainOrdinal(0);
                }
                return Ok(());
            }
        }
        self.next_path = None;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct OrdinalRun {
    start: u128,
    end_exclusive: u128,
    child: DecisionRef,
}

struct DimensionFrame {
    cardinality: u128,
    next_ordinal: u128,
    runs: Vec<OrdinalRun>,
}

impl DimensionFrame {
    fn new(cardinality: u128) -> Self {
        Self {
            cardinality,
            next_ordinal: 0,
            runs: Vec::new(),
        }
    }

    fn is_complete(&self) -> bool {
        self.next_ordinal == self.cardinality
    }

    fn push_one(&mut self, child: DecisionRef) -> Result<(), CaseGraphError> {
        if self.next_ordinal >= self.cardinality {
            return Err(CaseGraphError::InternalInvariant(
                "attempted to extend a complete decision frame",
            ));
        }
        let end_exclusive =
            self.next_ordinal
                .checked_add(1)
                .ok_or(CaseGraphError::InternalInvariant(
                    "decision frame ordinal overflowed",
                ))?;
        self.append_run(self.next_ordinal, end_exclusive, child);
        self.next_ordinal = end_exclusive;
        Ok(())
    }

    fn fill_remaining(&mut self, child: DecisionRef) -> Result<(), CaseGraphError> {
        if self.next_ordinal > self.cardinality {
            return Err(CaseGraphError::InternalInvariant(
                "decision frame advanced beyond its cardinality",
            ));
        }
        if self.next_ordinal < self.cardinality {
            self.append_run(self.next_ordinal, self.cardinality, child);
            self.next_ordinal = self.cardinality;
        }
        Ok(())
    }

    fn append_run(&mut self, start: u128, end_exclusive: u128, child: DecisionRef) {
        if let Some(last) = self.runs.last_mut() {
            if last.child == child && last.end_exclusive == start {
                last.end_exclusive = end_exclusive;
                return;
            }
        }
        self.runs.push(OrdinalRun {
            start,
            end_exclusive,
            child,
        });
    }

    fn take_complete(&mut self) -> Result<Vec<OrdinalRun>, CaseGraphError> {
        if !self.is_complete() || self.runs.is_empty() {
            return Err(CaseGraphError::InternalInvariant(
                "attempted to resolve an incomplete decision frame",
            ));
        }
        self.next_ordinal = 0;
        Ok(std::mem::take(&mut self.runs))
    }
}

fn checked_product(factors: &[u128]) -> CheckedCardinality {
    if factors.contains(&0) {
        return CheckedCardinality::Exact(0);
    }
    factors
        .iter()
        .copied()
        .fold(CheckedCardinality::Exact(1), |product, factor| {
            product.checked_mul(CheckedCardinality::Exact(factor))
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CaseGraphError {
    NoRemainingPath,
    UnexpectedPath {
        expected: Vec<u128>,
        actual: Vec<u128>,
    },
    IncompleteClassification {
        next: Vec<u128>,
    },
    PathArity {
        expected: usize,
        actual: usize,
    },
    OrdinalOutOfBounds {
        dimension: usize,
        ordinal: u128,
        cardinality: u128,
    },
    DuplicatePathConflict {
        path: Vec<u128>,
    },
    InvalidGraph(String),
    InternalInvariant(&'static str),
}

impl fmt::Display for CaseGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRemainingPath => write!(formatter, "the case graph has no remaining path"),
            Self::UnexpectedPath { expected, actual } => write!(
                formatter,
                "case classifications must be streamed in canonical order: expected {expected:?}, got {actual:?}"
            ),
            Self::IncompleteClassification { next } => write!(
                formatter,
                "the case graph is incomplete; the next unclassified path is {next:?}"
            ),
            Self::PathArity { expected, actual } => write!(
                formatter,
                "case path has {actual} ordinals, expected {expected}"
            ),
            Self::OrdinalOutOfBounds {
                dimension,
                ordinal,
                cardinality,
            } => write!(
                formatter,
                "ordinal {ordinal} is outside dimension {dimension} with cardinality {cardinality}"
            ),
            Self::DuplicatePathConflict { path } => write!(
                formatter,
                "case path {path:?} was classified with conflicting terminals"
            ),
            Self::InvalidGraph(message) => write!(formatter, "invalid case graph: {message}"),
            Self::InternalInvariant(message) => {
                write!(formatter, "case graph builder invariant failed: {message}")
            }
        }
    }
}

impl Error for CaseGraphError {}

impl From<CaseGraphError> for CaseRankRunLoweringError {
    fn from(error: CaseGraphError) -> Self {
        Self::Graph(error)
    }
}

impl fmt::Display for CaseRankRunLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOpenComplement => formatter.write_str(
                "rank-run case lowering requires an eligibility-open or admissible-open complement",
            ),
            Self::UniverseCardinalityOverflow => formatter.write_str(
                "rank-run case lowering requires a mixed-radix universe that fits in u128",
            ),
            Self::RunInEmptyUniverse { index } => {
                write!(formatter, "rank run {index} is present in an empty case universe")
            }
            Self::EmptyOrReversedRun {
                index,
                start,
                end_exclusive,
            } => write!(
                formatter,
                "rank run {index} has empty or reversed bounds [{start}, {end_exclusive})"
            ),
            Self::UnsortedOrOverlappingRun {
                index,
                previous_end_exclusive,
                start,
            } => write!(
                formatter,
                "rank run {index} starts at {start} before the previous end {previous_end_exclusive}"
            ),
            Self::RunOutOfBounds {
                index,
                end_exclusive,
                universe,
            } => write!(
                formatter,
                "rank run {index} ends at {end_exclusive}, outside universe cardinality {universe}"
            ),
            Self::LimitExceeded {
                resource,
                observed,
                limit,
            } => write!(
                formatter,
                "rank-run case lowering exceeded its {resource:?} limit: observed {observed}, limit {limit}"
            ),
            Self::AllocationFailed { resource } => write!(
                formatter,
                "rank-run case lowering could not reserve bounded {resource:?} storage"
            ),
            Self::ArithmeticOverflow(context) => {
                write!(formatter, "rank-run case lowering overflowed {context}")
            }
            Self::TerminalCountOverflow => {
                formatter.write_str("rank-run terminal counts exceed u128")
            }
            Self::TerminalCountMismatch { expected, actual } => write!(
                formatter,
                "rank-run terminal counts disagree with the lowered graph: expected {expected:?}, got {actual:?}"
            ),
            Self::Graph(error) => write!(formatter, "rank-run case graph is invalid: {error}"),
            Self::InternalInvariant(message) => {
                write!(formatter, "rank-run lowering invariant failed: {message}")
            }
        }
    }
}

impl Error for CaseRankRunLoweringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum TestTerminal {
        Excluded,
        Match,
        Nonmatch,
        Open,
    }

    fn raw(path: &CaseOrdinalPath) -> Vec<u128> {
        path.to_raw_ordinals()
    }

    fn case_open() -> CaseTerminal {
        CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted)
    }

    fn case_run(start: u128, end_exclusive: u128, terminal: CaseTerminal) -> CaseTerminalRankRun {
        CaseTerminalRankRun::new(start, end_exclusive, terminal)
    }

    fn path_for_rank(axis_cardinalities: &[u128], mut rank: u128) -> Vec<u128> {
        let mut suffix = vec![1_u128; axis_cardinalities.len() + 1];
        for dimension in (0..axis_cardinalities.len()).rev() {
            suffix[dimension] = axis_cardinalities[dimension] * suffix[dimension + 1];
        }
        assert!(rank < suffix[0]);
        let mut path = Vec::with_capacity(axis_cardinalities.len());
        for dimension in 0..axis_cardinalities.len() {
            let stride = suffix[dimension + 1];
            path.push(rank / stride);
            rank %= stride;
        }
        path
    }

    fn terminal_for_rank(
        graph: &CaseDecisionDag,
        rank: u128,
    ) -> Result<&CaseTerminal, CaseGraphError> {
        graph
            .terminal_for_path(&path_for_rank(graph.axis_cardinalities(), rank))?
            .ok_or(CaseGraphError::InternalInvariant(
                "nonempty rank has no case terminal",
            ))
    }

    #[test]
    fn rank_runs_match_the_canonical_singleton_builder_on_a_tiny_product() {
        let axes = vec![2, 3];
        let open = case_open();
        let runs = vec![
            case_run(1, 3, CaseTerminal::AdmissibleMatch),
            case_run(4, 5, CaseTerminal::Excluded),
        ];
        let lowered = lower_case_terminal_rank_runs(axes.clone(), runs, open.clone()).unwrap();

        let mut singleton = CaseGraphBuilder::new(axes);
        for terminal in [
            open.clone(),
            CaseTerminal::AdmissibleMatch,
            CaseTerminal::AdmissibleMatch,
            open.clone(),
            CaseTerminal::Excluded,
            open,
        ] {
            singleton.push_next(terminal).unwrap();
        }
        let singleton = singleton.finish_complete().unwrap();
        assert_eq!(lowered, singleton);
    }

    #[test]
    fn one_uniform_run_can_cover_the_full_u128_rank_universe() {
        let graph = lower_case_terminal_rank_runs(
            vec![u128::MAX],
            [case_run(0, u128::MAX, CaseTerminal::AdmissibleMatch)],
            case_open(),
        )
        .unwrap();
        assert!(matches!(
            graph.root(),
            DecisionRoot::Target(DecisionRef::Terminal(_))
        ));
        assert!(graph.nodes().is_empty());
        assert_eq!(
            graph.terminal_counts().unwrap(),
            BTreeMap::from([(
                CaseTerminal::AdmissibleMatch,
                CheckedCardinality::Exact(u128::MAX),
            )])
        );
    }

    #[test]
    fn aligned_and_unaligned_rank_runs_lower_only_boundary_pierced_subtrees() {
        let axes = vec![3, 4, 5];
        let open = case_open();
        let aligned = lower_case_terminal_rank_runs(
            axes.clone(),
            [case_run(20, 40, CaseTerminal::AdmissibleMatch)],
            open.clone(),
        )
        .unwrap();
        assert_eq!(aligned.nodes().len(), 1);
        assert_eq!(
            aligned.terminal_counts().unwrap(),
            BTreeMap::from([
                (open.clone(), CheckedCardinality::Exact(40),),
                (CaseTerminal::AdmissibleMatch, CheckedCardinality::Exact(20),),
            ])
        );

        let unaligned = lower_case_terminal_rank_runs(
            axes,
            [case_run(19, 41, CaseTerminal::AdmissibleMatch)],
            open.clone(),
        )
        .unwrap();
        assert!(unaligned.nodes().len() <= 5);
        assert_eq!(terminal_for_rank(&unaligned, 18).unwrap(), &open);
        assert_eq!(
            terminal_for_rank(&unaligned, 19).unwrap(),
            &CaseTerminal::AdmissibleMatch
        );
        assert_eq!(
            terminal_for_rank(&unaligned, 40).unwrap(),
            &CaseTerminal::AdmissibleMatch
        );
        assert_eq!(terminal_for_rank(&unaligned, 41).unwrap(), &open);
        assert_eq!(
            unaligned.terminal_counts().unwrap(),
            BTreeMap::from([
                (open, CheckedCardinality::Exact(38)),
                (CaseTerminal::AdmissibleMatch, CheckedCardinality::Exact(22),),
            ])
        );
    }

    #[test]
    fn rank_run_gaps_are_open_and_adjacent_equal_runs_normalize_identically() {
        let open = case_open();
        let split = lower_case_terminal_rank_runs(
            vec![10],
            [
                case_run(2, 3, CaseTerminal::AdmissibleMatch),
                case_run(3, 4, CaseTerminal::AdmissibleMatch),
                case_run(6, 8, CaseTerminal::AdmissibleNonmatch),
            ],
            open.clone(),
        )
        .unwrap();
        let coalesced = lower_case_terminal_rank_runs(
            vec![10],
            [
                case_run(2, 4, CaseTerminal::AdmissibleMatch),
                case_run(6, 8, CaseTerminal::AdmissibleNonmatch),
            ],
            open.clone(),
        )
        .unwrap();
        assert_eq!(split, coalesced);
        assert_eq!(
            split.terminal_counts().unwrap(),
            BTreeMap::from([
                (open.clone(), CheckedCardinality::Exact(6)),
                (
                    CaseTerminal::AdmissibleNonmatch,
                    CheckedCardinality::Exact(2),
                ),
                (CaseTerminal::AdmissibleMatch, CheckedCardinality::Exact(2),),
            ])
        );
        for rank in [0, 1, 4, 5, 8, 9] {
            assert_eq!(terminal_for_rank(&split, rank).unwrap(), &open);
        }
    }

    #[test]
    fn rank_run_lowering_distinguishes_zero_axis_singletons_and_empty_axes() {
        let open = case_open();
        let singleton =
            lower_case_terminal_rank_runs(Vec::new(), std::iter::empty(), open.clone()).unwrap();
        assert_eq!(
            singleton.terminal_counts().unwrap(),
            BTreeMap::from([(open.clone(), CheckedCardinality::Exact(1))])
        );
        let classified_singleton = lower_case_terminal_rank_runs(
            Vec::new(),
            [case_run(0, 1, CaseTerminal::AdmissibleMatch)],
            open.clone(),
        )
        .unwrap();
        assert_eq!(
            classified_singleton.terminal_counts().unwrap(),
            BTreeMap::from([(CaseTerminal::AdmissibleMatch, CheckedCardinality::Exact(1),)])
        );

        let empty =
            lower_case_terminal_rank_runs(vec![2, 0, 3], std::iter::empty(), open.clone()).unwrap();
        assert_eq!(empty.root(), DecisionRoot::EmptySpace);
        assert!(empty.nodes().is_empty());
        assert!(empty.terminals().is_empty());
        assert!(matches!(
            lower_case_terminal_rank_runs(
                vec![2, 0, 3],
                [case_run(0, 1, CaseTerminal::AdmissibleMatch)],
                open,
            ),
            Err(CaseRankRunLoweringError::RunInEmptyUniverse { index: 0 })
        ));
    }

    #[test]
    fn rank_run_lowering_rejects_malformed_streams_and_overflowing_universes() {
        let open = case_open();
        assert_eq!(
            lower_case_terminal_rank_runs(
                vec![2],
                std::iter::empty(),
                CaseTerminal::AdmissibleMatch,
            )
            .unwrap_err(),
            CaseRankRunLoweringError::InvalidOpenComplement
        );
        assert_eq!(
            lower_case_terminal_rank_runs(vec![u128::MAX, 2], std::iter::empty(), open.clone(),)
                .unwrap_err(),
            CaseRankRunLoweringError::UniverseCardinalityOverflow
        );
        assert!(matches!(
            lower_case_terminal_rank_runs(
                vec![6],
                [case_run(2, 2, CaseTerminal::AdmissibleMatch)],
                open.clone(),
            ),
            Err(CaseRankRunLoweringError::EmptyOrReversedRun { index: 0, .. })
        ));
        assert!(matches!(
            lower_case_terminal_rank_runs(
                vec![6],
                [
                    case_run(3, 5, CaseTerminal::AdmissibleMatch),
                    case_run(4, 6, CaseTerminal::AdmissibleNonmatch),
                ],
                open.clone(),
            ),
            Err(CaseRankRunLoweringError::UnsortedOrOverlappingRun { index: 1, .. })
        ));
        assert!(matches!(
            lower_case_terminal_rank_runs(
                vec![6],
                [
                    case_run(4, 5, CaseTerminal::AdmissibleMatch),
                    case_run(1, 2, CaseTerminal::AdmissibleNonmatch),
                ],
                open.clone(),
            ),
            Err(CaseRankRunLoweringError::UnsortedOrOverlappingRun { index: 1, .. })
        ));
        assert!(matches!(
            lower_case_terminal_rank_runs(
                vec![6],
                [case_run(5, 7, CaseTerminal::AdmissibleMatch)],
                open,
            ),
            Err(CaseRankRunLoweringError::RunOutOfBounds { index: 0, .. })
        ));
    }

    #[test]
    fn rank_run_limits_are_inclusive_and_fail_on_the_next_materialized_unit() {
        let open = case_open();
        let mut axis_limits = CaseRankRunLoweringLimits::default();
        axis_limits.max_axes = 1;
        lower_case_terminal_rank_runs_with_limits(
            vec![1],
            std::iter::empty(),
            open.clone(),
            axis_limits,
        )
        .unwrap();
        assert!(matches!(
            lower_case_terminal_rank_runs_with_limits(
                vec![1, 1],
                std::iter::empty(),
                open.clone(),
                axis_limits,
            ),
            Err(CaseRankRunLoweringError::LimitExceeded {
                resource: CaseRankRunLoweringResource::Axes,
                observed: 2,
                limit: 1,
            })
        ));

        let mut limits = CaseRankRunLoweringLimits::default();
        limits.max_runs = 1;
        lower_case_terminal_rank_runs_with_limits(
            vec![4],
            [case_run(1, 2, CaseTerminal::AdmissibleMatch)],
            open.clone(),
            limits,
        )
        .unwrap();
        assert!(matches!(
            lower_case_terminal_rank_runs_with_limits(
                vec![4],
                [
                    case_run(0, 1, CaseTerminal::AdmissibleMatch),
                    case_run(2, 3, CaseTerminal::AdmissibleNonmatch),
                ],
                open.clone(),
                limits,
            ),
            Err(CaseRankRunLoweringError::LimitExceeded {
                resource: CaseRankRunLoweringResource::Runs,
                observed: 2,
                limit: 1,
            })
        ));

        let one_middle_run = [case_run(1, 2, CaseTerminal::AdmissibleMatch)];
        let mut node_limits = CaseRankRunLoweringLimits::default();
        node_limits.max_nodes = 0;
        assert!(matches!(
            lower_case_terminal_rank_runs_with_limits(
                vec![3],
                one_middle_run.clone(),
                open.clone(),
                node_limits,
            ),
            Err(CaseRankRunLoweringError::LimitExceeded {
                resource: CaseRankRunLoweringResource::Nodes,
                observed: 1,
                limit: 0,
            })
        ));

        let mut arc_limits = CaseRankRunLoweringLimits::default();
        arc_limits.max_nodes = 1;
        arc_limits.max_arcs = 2;
        lower_case_terminal_rank_runs_with_limits(
            vec![3],
            one_middle_run.clone(),
            open.clone(),
            arc_limits,
        )
        .unwrap();
        arc_limits.max_arcs = 1;
        assert!(matches!(
            lower_case_terminal_rank_runs_with_limits(
                vec![3],
                one_middle_run.clone(),
                open.clone(),
                arc_limits,
            ),
            Err(CaseRankRunLoweringError::LimitExceeded {
                resource: CaseRankRunLoweringResource::Arcs,
                observed: 2,
                limit: 1,
            })
        ));

        let mut interval_limits = CaseRankRunLoweringLimits::default();
        interval_limits.max_nodes = 1;
        interval_limits.max_arcs = 2;
        interval_limits.max_ordinal_intervals = 3;
        lower_case_terminal_rank_runs_with_limits(
            vec![3],
            one_middle_run.clone(),
            open.clone(),
            interval_limits,
        )
        .unwrap();
        interval_limits.max_ordinal_intervals = 2;
        assert!(matches!(
            lower_case_terminal_rank_runs_with_limits(
                vec![3],
                one_middle_run,
                open.clone(),
                interval_limits,
            ),
            Err(CaseRankRunLoweringError::LimitExceeded {
                resource: CaseRankRunLoweringResource::OrdinalIntervals,
                observed: 3,
                limit: 2,
            })
        ));

        let mut byte_limits = CaseRankRunLoweringLimits::default();
        byte_limits.max_accounted_bytes =
            ACCOUNTED_STRIDE_BYTES + ACCOUNTED_NORMALIZED_SEGMENT_BYTES + ACCOUNTED_TERMINAL_BYTES;
        lower_case_terminal_rank_runs_with_limits(
            Vec::new(),
            std::iter::empty(),
            open.clone(),
            byte_limits,
        )
        .unwrap();
        byte_limits.max_accounted_bytes -= 1;
        assert!(matches!(
            lower_case_terminal_rank_runs_with_limits(
                Vec::new(),
                std::iter::empty(),
                open,
                byte_limits,
            ),
            Err(CaseRankRunLoweringError::LimitExceeded {
                resource: CaseRankRunLoweringResource::AccountedBytes,
                ..
            })
        ));
    }

    #[test]
    fn repeated_equal_suffixes_charge_only_retained_dag_units_against_shape_caps() {
        let open = case_open();
        let limits = CaseRankRunLoweringLimits::new(
            2,
            2,
            1,
            2,
            2,
            DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES,
        );
        let graph = lower_case_terminal_rank_runs_with_limits(
            vec![2, 2],
            [
                case_run(0, 1, CaseTerminal::AdmissibleMatch),
                case_run(2, 3, CaseTerminal::AdmissibleMatch),
            ],
            open,
            limits,
        )
        .unwrap();

        assert_eq!(graph.nodes().len(), 1);
        assert_eq!(graph.nodes()[0].arcs().len(), 2);
        assert_eq!(
            graph.nodes()[0]
                .arcs()
                .iter()
                .map(|arc| arc.ordinals().intervals().len())
                .sum::<usize>(),
            2
        );
    }

    #[test]
    fn complete_graph_shares_equal_suffixes_and_removes_irrelevant_axes() {
        let mut builder = OrderedDecisionDagBuilder::new(vec![2, 3]);
        let row = [
            TestTerminal::Nonmatch,
            TestTerminal::Match,
            TestTerminal::Nonmatch,
        ];
        for _ in 0..2 {
            for terminal in &row {
                builder.push_next(terminal.clone()).unwrap();
            }
        }

        let graph = builder.finish_complete().unwrap();
        let DecisionRoot::Target(DecisionRef::Node(root)) = graph.root() else {
            panic!("expected one reduced decision node");
        };
        assert_eq!(graph.nodes().len(), 1);
        assert_eq!(graph.node(root).unwrap().dimension_index(), 1);
        assert_eq!(
            graph.terminal_counts().unwrap(),
            BTreeMap::from([
                (TestTerminal::Match, CheckedCardinality::Exact(2)),
                (TestTerminal::Nonmatch, CheckedCardinality::Exact(4)),
            ])
        );
    }

    #[test]
    fn disconnected_ordinal_runs_are_coalesced_without_widening() {
        let mut builder = OrderedDecisionDagBuilder::new(vec![5]);
        for terminal in [
            TestTerminal::Match,
            TestTerminal::Nonmatch,
            TestTerminal::Match,
            TestTerminal::Nonmatch,
            TestTerminal::Match,
        ] {
            builder.push_next(terminal).unwrap();
        }

        let graph = builder.finish_complete().unwrap();
        let DecisionRoot::Target(DecisionRef::Node(root)) = graph.root() else {
            panic!("alternating classifications require a node");
        };
        let node = graph.node(root).unwrap();
        let match_arc = node
            .arcs()
            .iter()
            .find(|arc| {
                graph.terminal(match arc.child() {
                    DecisionRef::Terminal(id) => id,
                    DecisionRef::Node(_) => panic!("expected terminal arc"),
                }) == Some(&TestTerminal::Match)
            })
            .unwrap();
        assert_eq!(
            match_arc
                .ordinals()
                .intervals()
                .iter()
                .map(|interval| (interval.start().get(), interval.end_exclusive().get()))
                .collect::<Vec<_>>(),
            vec![(0, 1), (2, 3), (4, 5)]
        );
    }

    #[test]
    fn partial_finish_classifies_the_untouched_suffix_as_open() {
        let mut builder = OrderedDecisionDagBuilder::new(vec![2, 3]);
        for terminal in [
            TestTerminal::Nonmatch,
            TestTerminal::Nonmatch,
            TestTerminal::Match,
            TestTerminal::Match,
        ] {
            builder.push_next(terminal).unwrap();
        }
        assert_eq!(raw(builder.next_path().unwrap()), vec![1, 1]);

        let graph = builder.finish_with_remainder(TestTerminal::Open).unwrap();
        for (path, expected) in [
            (vec![0, 0], TestTerminal::Nonmatch),
            (vec![0, 1], TestTerminal::Nonmatch),
            (vec![0, 2], TestTerminal::Match),
            (vec![1, 0], TestTerminal::Match),
            (vec![1, 1], TestTerminal::Open),
            (vec![1, 2], TestTerminal::Open),
        ] {
            assert_eq!(graph.terminal_for_path(&path).unwrap(), Some(&expected));
        }
        assert_eq!(
            graph.terminal_counts().unwrap(),
            BTreeMap::from([
                (TestTerminal::Match, CheckedCardinality::Exact(2)),
                (TestTerminal::Nonmatch, CheckedCardinality::Exact(2)),
                (TestTerminal::Open, CheckedCardinality::Exact(2)),
            ])
        );
    }

    #[test]
    fn empty_axis_and_zero_axis_products_have_distinct_roots() {
        let empty = OrderedDecisionDagBuilder::<TestTerminal>::new(vec![2, 0, 3])
            .finish_complete()
            .unwrap();
        assert_eq!(empty.root(), DecisionRoot::EmptySpace);
        assert!(empty.nodes().is_empty());
        assert!(empty.terminals().is_empty());
        assert_eq!(empty.universe_cardinality(), CheckedCardinality::Exact(0));
        assert!(matches!(
            empty.terminal_for_path(&[0, 0, 0]),
            Err(CaseGraphError::OrdinalOutOfBounds {
                dimension: 1,
                ordinal: 0,
                cardinality: 0,
            })
        ));

        let mut singleton = OrderedDecisionDagBuilder::new(Vec::new());
        assert_eq!(raw(singleton.next_path().unwrap()), Vec::<u128>::new());
        singleton.push_next(TestTerminal::Match).unwrap();
        let singleton = singleton.finish_complete().unwrap();
        assert!(matches!(
            singleton.root(),
            DecisionRoot::Target(DecisionRef::Terminal(_))
        ));
        assert_eq!(
            singleton.terminal_counts().unwrap(),
            BTreeMap::from([(TestTerminal::Match, CheckedCardinality::Exact(1))])
        );

        let singleton_open = OrderedDecisionDagBuilder::new(Vec::new())
            .finish_with_remainder(TestTerminal::Open)
            .unwrap();
        assert_eq!(
            singleton_open.terminal_counts().unwrap(),
            BTreeMap::from([(TestTerminal::Open, CheckedCardinality::Exact(1))])
        );
    }

    #[test]
    fn canonical_streams_produce_deterministic_interning() {
        fn build() -> OrderedDecisionDag<TestTerminal> {
            let mut builder = OrderedDecisionDagBuilder::new(vec![2, 3]);
            for terminal in [
                TestTerminal::Match,
                TestTerminal::Nonmatch,
                TestTerminal::Match,
                TestTerminal::Nonmatch,
                TestTerminal::Match,
                TestTerminal::Nonmatch,
            ] {
                builder.push_next(terminal).unwrap();
            }
            builder.finish_complete().unwrap()
        }

        assert_eq!(build(), build());
    }

    #[test]
    fn contextual_counts_handle_a_shared_suffix_reached_after_different_skips() {
        let mut builder = OrderedDecisionDagBuilder::new(vec![2, 2, 2]);
        for terminal in [
            // axis 0 = 0: axis 1 is irrelevant, both paths share the same
            // axis-2 suffix.
            TestTerminal::Match,
            TestTerminal::Nonmatch,
            TestTerminal::Match,
            TestTerminal::Nonmatch,
            // axis 0 = 1: the first axis-1 value reuses that suffix, while the
            // second is uniformly matching.
            TestTerminal::Match,
            TestTerminal::Nonmatch,
            TestTerminal::Match,
            TestTerminal::Match,
        ] {
            builder.push_next(terminal).unwrap();
        }
        let graph = builder.finish_complete().unwrap();
        assert_eq!(
            graph.terminal_counts().unwrap(),
            BTreeMap::from([
                (TestTerminal::Match, CheckedCardinality::Exact(5)),
                (TestTerminal::Nonmatch, CheckedCardinality::Exact(3)),
            ])
        );
        graph.validate().unwrap();
    }

    #[test]
    fn canonical_order_is_enforced() {
        let mut builder = OrderedDecisionDagBuilder::new(vec![2, 2]);
        assert!(matches!(
            builder.classify(&[0, 1], TestTerminal::Match),
            Err(CaseGraphError::UnexpectedPath { .. })
        ));
        builder.classify(&[0, 0], TestTerminal::Match).unwrap();
        assert_eq!(raw(builder.next_path().unwrap()), vec![0, 1]);
        assert!(matches!(
            builder.finish_complete(),
            Err(CaseGraphError::IncompleteClassification { .. })
        ));
    }

    #[test]
    fn sparse_scattered_points_leave_a_three_million_case_complement_compact() {
        let classifications = vec![
            (vec![2_999_999], TestTerminal::Match),
            (vec![7], TestTerminal::Match),
            (vec![1_500_000], TestTerminal::Match),
        ];
        let graph = OrderedDecisionDag::from_sparse_classifications(
            vec![3_000_000],
            classifications,
            TestTerminal::Open,
        )
        .unwrap();

        assert_eq!(graph.nodes().len(), 1);
        assert_eq!(graph.terminals().len(), 2);
        assert_eq!(
            graph.terminal_counts().unwrap(),
            BTreeMap::from([
                (TestTerminal::Match, CheckedCardinality::Exact(3)),
                (TestTerminal::Open, CheckedCardinality::Exact(2_999_997)),
            ])
        );
        for ordinal in [7, 1_500_000, 2_999_999] {
            assert_eq!(
                graph.terminal_for_path(&[ordinal]).unwrap(),
                Some(&TestTerminal::Match)
            );
        }
        assert_eq!(
            graph.terminal_for_path(&[1_499_999]).unwrap(),
            Some(&TestTerminal::Open)
        );
    }

    #[test]
    fn sparse_multi_axis_points_preserve_correlations_and_reduce_shared_suffixes() {
        let correlated = OrderedDecisionDag::from_sparse_classifications(
            vec![3, 4],
            vec![
                (vec![2, 3], TestTerminal::Match),
                (vec![0, 1], TestTerminal::Match),
            ],
            TestTerminal::Open,
        )
        .unwrap();
        assert_eq!(
            correlated.terminal_counts().unwrap(),
            BTreeMap::from([
                (TestTerminal::Match, CheckedCardinality::Exact(2)),
                (TestTerminal::Open, CheckedCardinality::Exact(10)),
            ])
        );
        for path in [[0, 1], [2, 3]] {
            assert_eq!(
                correlated.terminal_for_path(&path).unwrap(),
                Some(&TestTerminal::Match)
            );
        }
        for path in [[0, 3], [1, 1], [2, 1]] {
            assert_eq!(
                correlated.terminal_for_path(&path).unwrap(),
                Some(&TestTerminal::Open)
            );
        }

        let shared_suffix = OrderedDecisionDag::from_sparse_classifications(
            vec![2, 3],
            vec![
                (vec![1, 1], TestTerminal::Match),
                (vec![0, 1], TestTerminal::Match),
            ],
            TestTerminal::Open,
        )
        .unwrap();
        let DecisionRoot::Target(DecisionRef::Node(root)) = shared_suffix.root() else {
            panic!("the shared suffix should retain one decision node");
        };
        assert_eq!(shared_suffix.nodes().len(), 1);
        assert_eq!(shared_suffix.node(root).unwrap().dimension_index(), 1);
        shared_suffix.validate().unwrap();
    }

    #[test]
    fn sparse_a_b_a_pattern_reuses_one_terminal_and_coalesces_its_intervals() {
        let graph = OrderedDecisionDag::from_sparse_classifications(
            vec![3],
            vec![(vec![1], TestTerminal::Match)],
            TestTerminal::Open,
        )
        .unwrap();
        let DecisionRoot::Target(DecisionRef::Node(root)) = graph.root() else {
            panic!("the A-B-A classification should retain one decision node");
        };
        let node = graph.node(root).unwrap();
        let open_arc = node
            .arcs()
            .iter()
            .find(|arc| {
                graph.terminal(match arc.child() {
                    DecisionRef::Terminal(id) => id,
                    DecisionRef::Node(_) => panic!("expected terminal arc"),
                }) == Some(&TestTerminal::Open)
            })
            .unwrap();
        assert_eq!(
            open_arc
                .ordinals()
                .intervals()
                .iter()
                .map(|interval| (interval.start().get(), interval.end_exclusive().get()))
                .collect::<Vec<_>>(),
            vec![(0, 1), (2, 3)]
        );
        assert_eq!(graph.terminals().len(), 2);
    }

    #[test]
    fn sparse_duplicate_paths_must_not_disagree() {
        let error = OrderedDecisionDag::from_sparse_classifications(
            vec![2],
            vec![
                (vec![1], TestTerminal::Match),
                (vec![1], TestTerminal::Nonmatch),
            ],
            TestTerminal::Open,
        )
        .unwrap_err();
        assert_eq!(
            error,
            CaseGraphError::DuplicatePathConflict { path: vec![1] }
        );

        let repeated = OrderedDecisionDag::from_sparse_classifications(
            vec![2],
            vec![
                (vec![1], TestTerminal::Match),
                (vec![1], TestTerminal::Match),
            ],
            TestTerminal::Open,
        )
        .unwrap();
        assert_eq!(
            repeated.terminal_counts().unwrap(),
            BTreeMap::from([
                (TestTerminal::Match, CheckedCardinality::Exact(1)),
                (TestTerminal::Open, CheckedCardinality::Exact(1)),
            ])
        );
    }

    #[test]
    fn sparse_paths_are_checked_for_arity_and_bounds() {
        let arity = OrderedDecisionDag::from_sparse_classifications(
            vec![2, 3],
            vec![(vec![1], TestTerminal::Match)],
            TestTerminal::Open,
        )
        .unwrap_err();
        assert_eq!(
            arity,
            CaseGraphError::PathArity {
                expected: 2,
                actual: 1,
            }
        );

        let bounds = OrderedDecisionDag::from_sparse_classifications(
            vec![2, 3],
            vec![(vec![2, 0], TestTerminal::Match)],
            TestTerminal::Open,
        )
        .unwrap_err();
        assert_eq!(
            bounds,
            CaseGraphError::OrdinalOutOfBounds {
                dimension: 0,
                ordinal: 2,
                cardinality: 2,
            }
        );
    }

    #[test]
    fn sparse_empty_axis_and_zero_axis_spaces_keep_their_distinct_meanings() {
        let empty = OrderedDecisionDag::from_sparse_classifications(
            vec![2, 0, 3],
            std::iter::empty::<(Vec<u128>, TestTerminal)>(),
            TestTerminal::Open,
        )
        .unwrap();
        assert_eq!(empty.root(), DecisionRoot::EmptySpace);
        assert!(empty.terminals().is_empty());
        assert_eq!(empty.universe_cardinality(), CheckedCardinality::Exact(0));

        let empty_path = OrderedDecisionDag::from_sparse_classifications(
            vec![2, 0, 3],
            vec![(vec![0, 0, 0], TestTerminal::Match)],
            TestTerminal::Open,
        )
        .unwrap_err();
        assert!(matches!(
            empty_path,
            CaseGraphError::OrdinalOutOfBounds {
                dimension: 1,
                ordinal: 0,
                cardinality: 0,
            }
        ));

        let default_singleton = OrderedDecisionDag::from_sparse_classifications(
            Vec::new(),
            std::iter::empty::<(Vec<u128>, TestTerminal)>(),
            TestTerminal::Open,
        )
        .unwrap();
        assert_eq!(
            default_singleton.terminal_counts().unwrap(),
            BTreeMap::from([(TestTerminal::Open, CheckedCardinality::Exact(1))])
        );

        let classified_singleton = OrderedDecisionDag::from_sparse_classifications(
            Vec::new(),
            vec![(Vec::<u128>::new(), TestTerminal::Match)],
            TestTerminal::Open,
        )
        .unwrap();
        assert_eq!(
            classified_singleton.terminal_counts().unwrap(),
            BTreeMap::from([(TestTerminal::Match, CheckedCardinality::Exact(1))])
        );
        assert_eq!(classified_singleton.terminals().len(), 1);
    }

    #[test]
    fn validator_rejects_nonexhaustive_arcs() {
        let mut builder = OrderedDecisionDagBuilder::new(vec![2]);
        builder.push_next(TestTerminal::Match).unwrap();
        builder.push_next(TestTerminal::Nonmatch).unwrap();
        let mut graph = builder.finish_complete().unwrap();
        let DecisionRoot::Target(DecisionRef::Node(root)) = graph.root() else {
            panic!("expected decision node");
        };
        graph.nodes[root.index()].arcs[0].ordinals.intervals[0].end_exclusive = DomainOrdinal(0);
        assert!(matches!(
            graph.validate(),
            Err(CaseGraphError::InvalidGraph(_))
        ));
    }

    #[test]
    fn checked_counts_report_u128_overflow_without_a_big_integer_dependency() {
        let graph = OrderedDecisionDagBuilder::<TestTerminal>::new(vec![u128::MAX, 2])
            .finish_with_remainder(TestTerminal::Open)
            .unwrap();
        assert_eq!(
            graph.universe_cardinality(),
            CheckedCardinality::ExceedsU128
        );
        assert_eq!(
            graph.terminal_counts().unwrap(),
            BTreeMap::from([(TestTerminal::Open, CheckedCardinality::ExceedsU128)])
        );
    }

    #[test]
    fn all_case_terminal_variants_remain_distinct() {
        let terminals = [
            CaseTerminal::Excluded,
            CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted),
            CaseTerminal::EligibilityOpen(CaseOpenReason::EvaluationUnknown),
            CaseTerminal::AdmissibleNonmatch,
            CaseTerminal::AdmissibleMatch,
            CaseTerminal::AdmissibleOpen(CaseOpenReason::SearchBudgetExhausted),
            CaseTerminal::AdmissibleOpen(CaseOpenReason::EvaluationUnknown),
        ];
        assert_eq!(terminals.into_iter().collect::<BTreeSet<_>>().len(), 7);
        assert_ne!(TestTerminal::Excluded, TestTerminal::Open);
    }
}
