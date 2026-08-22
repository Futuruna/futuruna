//! Backend-neutral evidence contract for exact finite Explore execution.
//!
//! Value-bearing report types intentionally do not implement serialization.
//! Public JSON and typed `after` values are separate, allowlisted projections
//! over this crate-private evidence.

use std::collections::BTreeSet;

use super::{
    case_graph::{CaseDecisionDag, CaseTerminal, CheckedCardinality},
    ExploreGeneratorAxisRole, ExplorePolarity, ExploreValue,
};

/// Canonical identity of one declared configuration.
///
/// Each component is the canonical ordinal of one independently varied
/// dimension, in Context → Before → independent-After field order. Values and
/// structured axis descriptors live in the report schema and (when
/// authorized) the configuration ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreCaseId(Box<[u128]>);

impl ExploreCaseId {
    pub(crate) fn new(ordinals: impl Into<Box<[u128]>>) -> Self {
        Self(ordinals.into())
    }

    pub(crate) fn ordinals(&self) -> &[u128] {
        &self.0
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// Canonical identity of one projected finding.
///
/// Names are stored once in [`ExploreReportSchema`]. Keeping this distinct
/// from [`ExploreCaseId`] prevents a projected key count from being mistaken
/// for a configuration count.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExploreResultKey(Box<[ExploreValue]>);

impl ExploreResultKey {
    pub(crate) fn new(values: impl Into<Box<[ExploreValue]>>) -> Self {
        Self(values.into())
    }

    pub(crate) fn values(&self) -> &[ExploreValue] {
        &self.0
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// Closed post-aggregation selection over raw key groups.
///
/// This is report evidence rather than executable syntax so validators do not
/// need to trust or retain the source AST. Suppression never changes D, M, the
/// search decision DAG, or the optional matching-configuration ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreGroupFilter {
    All,
    Varies { extrema_index: usize },
}

/// Canonical descriptor of one independently varied generator axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreReportDimension {
    pub(crate) bound_index: usize,
    pub(crate) role: ExploreGeneratorAxisRole,
    pub(crate) role_field_index: usize,
    pub(crate) label: String,
}

impl ExploreReportDimension {
    pub(crate) fn qualified_label(&self) -> String {
        let role = match self.role {
            ExploreGeneratorAxisRole::Context => "context",
            ExploreGeneratorAxisRole::Before => "before",
            ExploreGeneratorAxisRole::AfterIndependent => "after",
        };
        format!("{role}.{}", self.label)
    }

    fn canonical_order_key(&self) -> (u8, usize, usize) {
        let role = match self.role {
            ExploreGeneratorAxisRole::Context => 0,
            ExploreGeneratorAxisRole::Before => 1,
            ExploreGeneratorAxisRole::AfterIndependent => 2,
        };
        (role, self.role_field_index, self.bound_index)
    }
}

/// Field identities and labels shared by every value-bearing row in one report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreReportSchema {
    pub(crate) dimensions: Box<[ExploreReportDimension]>,
    pub(crate) axis_cardinalities: Box<[u128]>,
    pub(crate) key_names: Box<[String]>,
    pub(crate) extrema_names: Box<[String]>,
    pub(crate) shown_names: Box<[String]>,
    pub(crate) group_filter: ExploreGroupFilter,
}

impl ExploreReportSchema {
    pub(crate) fn new(
        dimensions: impl Into<Box<[ExploreReportDimension]>>,
        axis_cardinalities: impl Into<Box<[u128]>>,
        key_names: impl Into<Box<[String]>>,
        shown_names: impl Into<Box<[String]>>,
    ) -> Result<Self, String> {
        let schema = Self {
            dimensions: dimensions.into(),
            axis_cardinalities: axis_cardinalities.into(),
            key_names: key_names.into(),
            extrema_names: Vec::<String>::new().into_boxed_slice(),
            shown_names: shown_names.into(),
            group_filter: ExploreGroupFilter::All,
        };
        schema.validate()?;
        Ok(schema)
    }

    pub(crate) fn with_grouped_extrema(
        dimensions: impl Into<Box<[ExploreReportDimension]>>,
        axis_cardinalities: impl Into<Box<[u128]>>,
        key_names: impl Into<Box<[String]>>,
        extrema_names: impl Into<Box<[String]>>,
        shown_names: impl Into<Box<[String]>>,
        group_filter: ExploreGroupFilter,
    ) -> Result<Self, String> {
        let schema = Self {
            dimensions: dimensions.into(),
            axis_cardinalities: axis_cardinalities.into(),
            key_names: key_names.into(),
            extrema_names: extrema_names.into(),
            shown_names: shown_names.into(),
            group_filter,
        };
        schema.validate()?;
        Ok(schema)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let mut roles = BTreeSet::new();
        let mut bound_indices = BTreeSet::new();
        for dimension in &self.dimensions {
            if dimension.label.is_empty() {
                return Err("Explore case dimension label must not be empty".to_string());
            }
            if !roles.insert((dimension.role, dimension.role_field_index)) {
                return Err(format!(
                    "Explore case dimension role {:?} field {} occurs more than once",
                    dimension.role, dimension.role_field_index
                ));
            }
            if !bound_indices.insert(dimension.bound_index) {
                return Err(format!(
                    "Explore case dimension bound {} occurs more than once",
                    dimension.bound_index
                ));
            }
        }
        if self
            .dimensions
            .windows(2)
            .any(|pair| pair[0].canonical_order_key() >= pair[1].canonical_order_key())
        {
            return Err(
                "Explore case dimensions are not in canonical Context → Before → independent-After field order"
                    .to_string(),
            );
        }
        if self.dimensions.len() != self.axis_cardinalities.len() {
            return Err(format!(
                "Explore schema has {} dimension descriptors but {} axis cardinalities",
                self.dimensions.len(),
                self.axis_cardinalities.len()
            ));
        }

        let mut output_names = BTreeSet::new();
        for (kind, names) in [
            ("key", self.key_names.as_ref()),
            ("extrema", self.extrema_names.as_ref()),
            ("shown", self.shown_names.as_ref()),
        ] {
            for name in names {
                if name.is_empty() {
                    return Err(format!("Explore {kind} field name must not be empty"));
                }
                if !output_names.insert(name.as_str()) {
                    return Err(format!(
                        "Explore output field name `{name}` occurs more than once"
                    ));
                }
            }
        }
        if let ExploreGroupFilter::Varies { extrema_index } = self.group_filter {
            if extrema_index >= self.extrema_names.len() {
                return Err(format!(
                    "Explore varies filter index {extrema_index} is outside {} extrema fields",
                    self.extrema_names.len()
                ));
            }
        }
        Ok(())
    }

    fn declared_assignment_count(&self) -> Result<u128, String> {
        if self.axis_cardinalities.contains(&0) {
            return Ok(0);
        }
        self.axis_cardinalities
            .iter()
            .copied()
            .try_fold(1_u128, u128::checked_mul)
            .ok_or_else(|| "Explore declared assignment count exceeds u128::MAX".to_string())
    }
}

fn validate_unique_names(kind: &str, names: &[String]) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for name in names {
        if name.is_empty() {
            return Err(format!("Explore {kind} name must not be empty"));
        }
        if !unique.insert(name.as_str()) {
            return Err(format!("Explore {kind} `{name}` occurs more than once"));
        }
    }
    Ok(())
}

/// Privacy-sensitive search decision DAG materialization requested for this
/// report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreSearchDecisionDagRequest {
    Omit,
    Include,
}

/// Privacy-sensitive semantic before/context/after graph materialization
/// requested for this report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreSemanticTransitionGraphRequest {
    Omit,
    Include,
}

/// Privacy-sensitive matching-configuration materialization requested for
/// this report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreLedgerRequest {
    Omit,
    MatchingConfigurations,
}

/// Explicit report shape. The baseline request publishes projected result
/// rows but neither graph nor the matching-case ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreReportRequest {
    pub(crate) search_decision_dag: ExploreSearchDecisionDagRequest,
    pub(crate) semantic_transition_graph: ExploreSemanticTransitionGraphRequest,
    pub(crate) ledger: ExploreLedgerRequest,
}

impl ExploreReportRequest {
    pub(crate) const fn baseline() -> Self {
        Self {
            search_decision_dag: ExploreSearchDecisionDagRequest::Omit,
            semantic_transition_graph: ExploreSemanticTransitionGraphRequest::Omit,
            ledger: ExploreLedgerRequest::Omit,
        }
    }
}

impl Default for ExploreReportRequest {
    fn default() -> Self {
        Self::baseline()
    }
}

/// Search decision DAG evidence, kept separate from the request so a report
/// cannot silently expand its disclosure surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreSearchDecisionDagEvidence {
    Omitted,
    Included(CaseDecisionDag),
}

/// Every publicly exposed row has passed a fresh ordinary-interpreter replay.
/// Failed or pending replay is never representable as a public row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreReplayStatus {
    Confirmed,
}

/// Exact closed extrema for one named integer measure in one result-key
/// group. Each endpoint carries its independently replay-confirmed canonical
/// witness. Tie supports make deterministic representative selection honest:
/// the chosen case is canonical, but it need not be the only optimum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreExtremaSummary {
    pub(crate) minimum: i64,
    pub(crate) maximum: i64,
    pub(crate) spread: u128,
    pub(crate) minimum_tie_support: u128,
    pub(crate) maximum_tie_support: u128,
    pub(crate) minimum_witness: ExploreCaseId,
    pub(crate) maximum_witness: ExploreCaseId,
}

/// One canonical projected result and its selected representative case.
///
/// The representative objective is deliberately absent. It remains internal
/// unless the query separately authorizes the same value as a key or shown
/// field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreResultRow {
    pub(crate) key: ExploreResultKey,
    pub(crate) extrema: Box<[ExploreExtremaSummary]>,
    pub(crate) shown: Box<[ExploreValue]>,
    pub(crate) representative: ExploreCaseId,
    /// Matching configurations observed in this projected key class. This is
    /// exact only when the matching population and projection are closed.
    pub(crate) support: ExploreCount,
    pub(crate) replay: ExploreReplayStatus,
}

impl ExploreResultRow {
    pub(crate) fn confirmed(
        key: ExploreResultKey,
        shown: impl Into<Box<[ExploreValue]>>,
        representative: ExploreCaseId,
    ) -> Self {
        Self {
            key,
            extrema: Vec::<ExploreExtremaSummary>::new().into_boxed_slice(),
            shown: shown.into(),
            representative,
            support: ExploreCount::Exact(1),
            replay: ExploreReplayStatus::Confirmed,
        }
    }

    pub(crate) fn confirmed_with_support(
        key: ExploreResultKey,
        shown: impl Into<Box<[ExploreValue]>>,
        representative: ExploreCaseId,
        support: ExploreCount,
    ) -> Self {
        Self {
            key,
            extrema: Vec::<ExploreExtremaSummary>::new().into_boxed_slice(),
            shown: shown.into(),
            representative,
            support,
            replay: ExploreReplayStatus::Confirmed,
        }
    }

    pub(crate) fn confirmed_with_support_and_extrema(
        key: ExploreResultKey,
        extrema: impl Into<Box<[ExploreExtremaSummary]>>,
        shown: impl Into<Box<[ExploreValue]>>,
        representative: ExploreCaseId,
        support: ExploreCount,
    ) -> Self {
        Self {
            key,
            extrema: extrema.into(),
            shown: shown.into(),
            representative,
            support,
            replay: ExploreReplayStatus::Confirmed,
        }
    }
}

/// One authorized row in the lossless matching-configuration ledger.
///
/// Only independently varied dimension values, key values and shown values
/// are present. Fixed facts, derived facts, objectives and interpreter state
/// remain private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreLedgerRow {
    pub(crate) case_id: ExploreCaseId,
    pub(crate) dimensions: Box<[ExploreValue]>,
    pub(crate) key: ExploreResultKey,
    pub(crate) shown: Box<[ExploreValue]>,
    pub(crate) replay: ExploreReplayStatus,
}

impl ExploreLedgerRow {
    pub(crate) fn confirmed(
        case_id: ExploreCaseId,
        dimensions: impl Into<Box<[ExploreValue]>>,
        key: ExploreResultKey,
        shown: impl Into<Box<[ExploreValue]>>,
    ) -> Self {
        Self {
            case_id,
            dimensions: dimensions.into(),
            key,
            shown: shown.into(),
            replay: ExploreReplayStatus::Confirmed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreLedgerEvidence {
    Omitted,
    MatchingConfigurations { rows: Box<[ExploreLedgerRow]> },
}

/// What is known about a nonnegative population count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreCount {
    Exact(u128),
    LowerBound(u128),
    Unknown,
}

impl ExploreCount {
    pub(crate) fn exact(self) -> Option<u128> {
        match self {
            Self::Exact(value) => Some(value),
            Self::LowerBound(_) | Self::Unknown => None,
        }
    }

    pub(crate) fn proven_lower_bound(self) -> Option<u128> {
        match self {
            Self::Exact(value) | Self::LowerBound(value) => Some(value),
            Self::Unknown => None,
        }
    }

    pub(crate) fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

/// Counts for the four distinct Explore populations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreCounts {
    /// `|U|`: declared Cartesian assignments before eligibility and `where`.
    pub(crate) declared_assignments: ExploreCount,
    /// `|D|`: admissible configurations.
    pub(crate) admissible_configurations: ExploreCount,
    /// `|M|`: matching admissible configurations.
    pub(crate) matching_configurations: ExploreCount,
    /// `|R|`: distinct emitted result keys after any closed group filter.
    pub(crate) distinct_result_keys: ExploreCount,
}

impl ExploreCounts {
    pub(crate) fn all_exact(self) -> bool {
        self.declared_assignments.is_exact()
            && self.admissible_configurations.is_exact()
            && self.matching_configurations.is_exact()
            && self.distinct_result_keys.is_exact()
    }

    fn validate(self) -> Result<(), String> {
        validate_count_subset(
            "admissible configurations",
            self.admissible_configurations,
            "declared assignments",
            self.declared_assignments,
        )?;
        validate_count_subset(
            "matching configurations",
            self.matching_configurations,
            "admissible configurations",
            self.admissible_configurations,
        )?;
        validate_count_subset(
            "distinct result keys",
            self.distinct_result_keys,
            "matching configurations",
            self.matching_configurations,
        )
    }
}

/// Grouped projection populations, kept separate from U/D/M/R so a `having`
/// filter cannot be mistaken for case reclassification.
///
/// `raw_groups` is G, `emitted_groups` is R, and the two configuration counts
/// are Q and S from the workbook conservation law Q + S = M.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreGroupCounts {
    pub(crate) raw_groups: ExploreCount,
    pub(crate) emitted_groups: ExploreCount,
    pub(crate) suppressed_groups: ExploreCount,
    pub(crate) qualifying_configurations: ExploreCount,
    pub(crate) suppressed_configurations: ExploreCount,
}

/// Auditable work accounting for the search order used by one exact run.
///
/// Source-event metadata is deliberately reduced to counts here. In
/// particular, labels are scheduling hints and cannot become mechanism
/// evidence merely because their candidates were evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreSearchEvidence {
    Canonical {
        classified_cases: u128,
        remaining_open_cases: u128,
        exhausted: bool,
    },
    SourceCandidateFirst {
        distinct_source_candidates: u128,
        scheduled_source_candidates: u128,
        evaluated_source_candidates: u128,
        scheduled_fallback_cases: u128,
        evaluated_fallback_cases: u128,
        singleton_closed_cases: u128,
        certified_region_closed_cases: u128,
        pending_evaluations: u128,
        remaining_open_cases: u128,
        exhausted: bool,
    },
}

impl ExploreSearchEvidence {
    fn validate(self, declared: u128) -> Result<(), String> {
        match self {
            Self::Canonical {
                classified_cases,
                remaining_open_cases,
                exhausted,
            } => {
                let accounted = classified_cases
                    .checked_add(remaining_open_cases)
                    .ok_or_else(|| {
                        "canonical Explore search accounting exceeds u128::MAX".to_string()
                    })?;
                if accounted != declared {
                    return Err(format!(
                        "canonical Explore search accounts for {accounted} cases, expected {declared}"
                    ));
                }
                if exhausted != (remaining_open_cases == 0) {
                    return Err(
                        "canonical Explore search is exhausted if and only if no case remains open"
                            .to_string(),
                    );
                }
            }
            Self::SourceCandidateFirst {
                distinct_source_candidates,
                scheduled_source_candidates,
                evaluated_source_candidates,
                scheduled_fallback_cases,
                evaluated_fallback_cases,
                singleton_closed_cases,
                certified_region_closed_cases,
                pending_evaluations,
                remaining_open_cases,
                exhausted,
            } => {
                if distinct_source_candidates > declared
                    || scheduled_source_candidates > distinct_source_candidates
                    || evaluated_source_candidates > scheduled_source_candidates
                    || evaluated_fallback_cases > scheduled_fallback_cases
                {
                    return Err(
                        "source-candidate Explore search scheduling counts are not monotone"
                            .to_string(),
                    );
                }
                let scheduled = scheduled_source_candidates
                    .checked_add(scheduled_fallback_cases)
                    .ok_or_else(|| {
                        "source-candidate Explore scheduled-work count exceeds u128::MAX"
                            .to_string()
                    })?;
                let evaluated = evaluated_source_candidates
                    .checked_add(evaluated_fallback_cases)
                    .ok_or_else(|| {
                        "source-candidate Explore evaluated-work count exceeds u128::MAX"
                            .to_string()
                    })?;
                if evaluated != singleton_closed_cases {
                    return Err(format!(
                        "source-candidate Explore evaluated work {evaluated} disagrees with singleton closure {singleton_closed_cases}"
                    ));
                }
                if scheduled.checked_sub(evaluated) != Some(pending_evaluations) {
                    return Err(
                        "source-candidate Explore pending work does not equal scheduled minus evaluated work"
                            .to_string(),
                    );
                }
                if pending_evaluations > remaining_open_cases {
                    return Err(
                        "source-candidate Explore pending work exceeds remaining open cases"
                            .to_string(),
                    );
                }
                let accounted = singleton_closed_cases
                    .checked_add(certified_region_closed_cases)
                    .and_then(|closed| closed.checked_add(remaining_open_cases))
                    .ok_or_else(|| {
                        "source-candidate Explore closure accounting exceeds u128::MAX".to_string()
                    })?;
                if accounted != declared {
                    return Err(format!(
                        "source-candidate Explore search accounts for {accounted} cases, expected {declared}"
                    ));
                }
                if exhausted != (remaining_open_cases == 0 && pending_evaluations == 0) {
                    return Err(
                        "source-candidate Explore search is exhausted if and only if no case remains open or pending"
                            .to_string(),
                    );
                }
            }
        }
        Ok(())
    }

    pub(crate) fn remaining_open_cases(self) -> u128 {
        match self {
            Self::Canonical {
                remaining_open_cases,
                ..
            }
            | Self::SourceCandidateFirst {
                remaining_open_cases,
                ..
            } => remaining_open_cases,
        }
    }
}

impl ExploreGroupCounts {
    pub(crate) fn unfiltered(groups: ExploreCount, matching_configurations: ExploreCount) -> Self {
        Self {
            raw_groups: groups,
            emitted_groups: groups,
            suppressed_groups: ExploreCount::Exact(0),
            qualifying_configurations: matching_configurations,
            suppressed_configurations: ExploreCount::Exact(0),
        }
    }

    fn all_exact(self) -> bool {
        self.raw_groups.is_exact()
            && self.emitted_groups.is_exact()
            && self.suppressed_groups.is_exact()
            && self.qualifying_configurations.is_exact()
            && self.suppressed_configurations.is_exact()
    }

    fn validate(self, counts: ExploreCounts, filter: ExploreGroupFilter) -> Result<(), String> {
        if self.emitted_groups != counts.distinct_result_keys {
            return Err(
                "Explore emitted-group count must equal the distinct result-key count".to_string(),
            );
        }
        validate_count_subset(
            "raw groups",
            self.raw_groups,
            "matching configurations",
            counts.matching_configurations,
        )?;
        validate_count_subset(
            "emitted groups",
            self.emitted_groups,
            "raw groups",
            self.raw_groups,
        )?;
        validate_count_subset(
            "suppressed groups",
            self.suppressed_groups,
            "raw groups",
            self.raw_groups,
        )?;
        validate_count_subset(
            "qualifying configurations",
            self.qualifying_configurations,
            "matching configurations",
            counts.matching_configurations,
        )?;
        validate_count_subset(
            "suppressed configurations",
            self.suppressed_configurations,
            "matching configurations",
            counts.matching_configurations,
        )?;
        validate_count_subset(
            "emitted groups",
            self.emitted_groups,
            "qualifying configurations",
            self.qualifying_configurations,
        )?;
        validate_count_subset(
            "suppressed groups",
            self.suppressed_groups,
            "suppressed configurations",
            self.suppressed_configurations,
        )?;

        if matches!(filter, ExploreGroupFilter::All) {
            if self.raw_groups != self.emitted_groups
                || self.suppressed_groups != ExploreCount::Exact(0)
                || self.qualifying_configurations != counts.matching_configurations
                || self.suppressed_configurations != ExploreCount::Exact(0)
            {
                return Err(
                    "unfiltered Explore group accounting must preserve every group and matching configuration"
                        .to_string(),
                );
            }
        }

        if self.all_exact() && counts.matching_configurations.is_exact() {
            let raw = self.raw_groups.exact().expect("all group counts are exact");
            let emitted = self
                .emitted_groups
                .exact()
                .expect("all group counts are exact");
            let suppressed = self
                .suppressed_groups
                .exact()
                .expect("all group counts are exact");
            if emitted.checked_add(suppressed) != Some(raw) {
                return Err(format!(
                    "Explore closed group accounting violates G = R + suppressed_groups: {raw} != {emitted} + {suppressed}"
                ));
            }
            let matching = counts
                .matching_configurations
                .exact()
                .expect("matching count is exact");
            let qualifying = self
                .qualifying_configurations
                .exact()
                .expect("all group counts are exact");
            let suppressed_configurations = self
                .suppressed_configurations
                .exact()
                .expect("all group counts are exact");
            if qualifying.checked_add(suppressed_configurations) != Some(matching) {
                return Err(format!(
                    "Explore closed group accounting violates Q + S = M: {qualifying} + {suppressed_configurations} != {matching}"
                ));
            }
        }
        Ok(())
    }
}

fn validate_count_subset(
    subset_name: &str,
    subset: ExploreCount,
    superset_name: &str,
    superset: ExploreCount,
) -> Result<(), String> {
    let (Some(subset_lower), Some(superset_exact)) =
        (subset.proven_lower_bound(), superset.exact())
    else {
        return Ok(());
    };
    if subset_lower > superset_exact {
        return Err(format!(
            "Explore {subset_name} lower bound {subset_lower} exceeds exact {superset_name} count {superset_exact}"
        ));
    }
    Ok(())
}

/// Matching coverage over the admissible population, independent of run
/// status and result-key projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreCoverage {
    Empty,
    None,
    Some,
    All,
    Undetermined,
}

impl ExploreCoverage {
    pub(crate) fn from_counts(admissible: ExploreCount, matching: ExploreCount) -> Self {
        let (Some(admissible), Some(matching)) = (admissible.exact(), matching.exact()) else {
            return Self::Undetermined;
        };
        match (admissible, matching) {
            (0, 0) => Self::Empty,
            (0, _) => Self::Undetermined,
            (_, 0) => Self::None,
            (admissible, matching) if admissible == matching => Self::All,
            (admissible, matching) if matching < admissible => Self::Some,
            _ => Self::Undetermined,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreClosure {
    Open,
    Closed,
}

impl ExploreClosure {
    pub(crate) fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

/// Closure of each answer/case/value layer. Mechanism closure is deliberately
/// not aggregated here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreLayerClosures {
    pub(crate) admissibility: ExploreClosure,
    pub(crate) polarity: ExploreClosure,
    /// Raw key discovery plus extrema aggregation and group filtering.
    pub(crate) projection: ExploreClosure,
    pub(crate) representatives: ExploreClosure,
    pub(crate) rows: ExploreClosure,
    pub(crate) views: ExploreClosure,
}

impl ExploreLayerClosures {
    pub(crate) const fn closed() -> Self {
        Self {
            admissibility: ExploreClosure::Closed,
            polarity: ExploreClosure::Closed,
            projection: ExploreClosure::Closed,
            representatives: ExploreClosure::Closed,
            rows: ExploreClosure::Closed,
            views: ExploreClosure::Closed,
        }
    }

    pub(crate) fn all_closed(self) -> bool {
        self.admissibility.is_closed()
            && self.polarity.is_closed()
            && self.projection.is_closed()
            && self.representatives.is_closed()
            && self.rows.is_closed()
            && self.views.is_closed()
    }
}

/// Semantic closure proof used by an exact terminal result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreCompletionMethod {
    ExactFiniteExhaustion,
    ExactFiniteCertifiedClosure,
}

impl ExploreCompletionMethod {
    fn validate_search(self, search: ExploreSearchEvidence) -> Result<(), String> {
        match (self, search) {
            (Self::ExactFiniteExhaustion, ExploreSearchEvidence::Canonical { .. })
            | (
                Self::ExactFiniteExhaustion,
                ExploreSearchEvidence::SourceCandidateFirst {
                    certified_region_closed_cases: 0,
                    ..
                },
            ) => Ok(()),
            (
                Self::ExactFiniteExhaustion,
                ExploreSearchEvidence::SourceCandidateFirst {
                    certified_region_closed_cases,
                    ..
                },
            ) => Err(format!(
                "exact finite exhaustion cannot claim {certified_region_closed_cases} cases closed by certified source regions"
            )),
            (
                Self::ExactFiniteCertifiedClosure,
                ExploreSearchEvidence::SourceCandidateFirst {
                    certified_region_closed_cases,
                    ..
                },
            ) if certified_region_closed_cases > 0 => Ok(()),
            (
                Self::ExactFiniteCertifiedClosure,
                ExploreSearchEvidence::SourceCandidateFirst { .. },
            ) => Err(
                "exact finite certified closure requires at least one case closed by a certified source region"
                    .to_string(),
            ),
            (Self::ExactFiniteCertifiedClosure, ExploreSearchEvidence::Canonical { .. }) => Err(
                "exact finite certified closure requires source-candidate search evidence"
                    .to_string(),
            ),
        }
    }
}

pub(crate) const DEFAULT_EXPLORE_STEP_LIMIT: usize = 4_000_000;
pub(crate) const DEFAULT_EXPLORE_COLLECTION_LIMIT: usize = 1_000_000;

/// Operational limits do not change query identity or the bounded world.
/// Runtime safety limits are always finite and positive. The optional case
/// limit is the only caller-selected answer-search cap in this exact slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreExecutionBudget {
    pub(crate) case_limit: Option<u128>,
    pub(crate) step_limit: usize,
    pub(crate) collection_limit: usize,
}

impl ExploreExecutionBudget {
    pub(crate) fn new(
        case_limit: Option<u128>,
        step_limit: usize,
        collection_limit: usize,
    ) -> Result<Self, String> {
        if step_limit == 0 {
            return Err("Explore runtime step limit must be positive".to_string());
        }
        if collection_limit == 0 {
            return Err("Explore runtime collection limit must be positive".to_string());
        }
        Ok(Self {
            case_limit,
            step_limit,
            collection_limit,
        })
    }
}

impl Default for ExploreExecutionBudget {
    fn default() -> Self {
        Self {
            case_limit: None,
            step_limit: DEFAULT_EXPLORE_STEP_LIMIT,
            collection_limit: DEFAULT_EXPLORE_COLLECTION_LIMIT,
        }
    }
}

/// Stable report-facing resource categories. Runtime-specific initialization
/// and expression step counters both map to `Steps`; the evaluation phase
/// retains where the limit occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreLimitResource {
    Steps,
    CollectionMembers { operation: String },
}

/// Query phase in which an operational runtime limit stopped exact
/// classification or required replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreEvaluationPhase {
    Initialization,
    DerivedFact { name: String },
    BoundaryEndpoint,
    Constraint { index: usize },
    Question,
    Key { name: String },
    Extrema { name: String },
    Show { name: String },
    Objective,
    Replay,
}

/// Why required answer/case/value closure stopped without a solver-unknown or
/// semantic-unsupported result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreStopReason {
    CaseLimit {
        limit: u128,
    },
    RuntimeLimit {
        resource: ExploreLimitResource,
        limit: u128,
        observed: u128,
        phase: ExploreEvaluationPhase,
    },
}

impl ExploreStopReason {
    fn validate(&self) -> Result<(), String> {
        if let Self::RuntimeLimit {
            resource,
            limit,
            observed,
            phase,
        } = self
        {
            if *limit == 0 {
                return Err("Explore runtime limit must be positive".to_string());
            }
            if observed <= limit {
                return Err(format!(
                    "Explore runtime limit {limit} was reported at non-exceeding observation {observed}"
                ));
            }
            validate_limit_resource(resource)?;
            validate_evaluation_phase(phase)?;
        }
        Ok(())
    }
}

fn validate_limit_resource(resource: &ExploreLimitResource) -> Result<(), String> {
    if let ExploreLimitResource::CollectionMembers { operation } = resource {
        if operation.is_empty() {
            return Err("Explore collection-limit operation must not be empty".to_string());
        }
    }
    Ok(())
}

fn validate_evaluation_phase(phase: &ExploreEvaluationPhase) -> Result<(), String> {
    let name = match phase {
        ExploreEvaluationPhase::DerivedFact { name }
        | ExploreEvaluationPhase::Key { name }
        | ExploreEvaluationPhase::Extrema { name }
        | ExploreEvaluationPhase::Show { name } => Some(name),
        ExploreEvaluationPhase::Initialization
        | ExploreEvaluationPhase::BoundaryEndpoint
        | ExploreEvaluationPhase::Constraint { .. }
        | ExploreEvaluationPhase::Question
        | ExploreEvaluationPhase::Objective
        | ExploreEvaluationPhase::Replay => None,
    };
    if name.is_some_and(String::is_empty) {
        return Err("Explore evaluation phase name must not be empty".to_string());
    }
    Ok(())
}

/// Mechanism analysis is intentionally deferred in the first exact executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreMechanismUnavailableReason {
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreMechanismEvidence {
    Unavailable {
        reason: ExploreMechanismUnavailableReason,
    },
}

impl ExploreMechanismEvidence {
    pub(crate) const fn deferred() -> Self {
        Self::Unavailable {
            reason: ExploreMechanismUnavailableReason::Deferred,
        }
    }
}

/// Closed or frontier-bearing evidence produced by a backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreExactEvidence {
    pub(crate) request: ExploreReportRequest,
    pub(crate) schema: ExploreReportSchema,
    pub(crate) search: ExploreSearchEvidence,
    pub(crate) counts: ExploreCounts,
    pub(crate) group_counts: ExploreGroupCounts,
    pub(crate) coverage: ExploreCoverage,
    pub(crate) closures: ExploreLayerClosures,
    pub(crate) results: Box<[ExploreResultRow]>,
    pub(crate) search_decision_dag: ExploreSearchDecisionDagEvidence,
    pub(crate) ledger: ExploreLedgerEvidence,
}

impl ExploreExactEvidence {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.schema.validate()?;
        self.counts.validate()?;
        self.group_counts
            .validate(self.counts, self.schema.group_filter)?;
        self.validate_declared_count()?;
        let declared = self
            .counts
            .declared_assignments
            .exact()
            .expect("validated Explore evidence always has exact U");
        self.search.validate(declared)?;
        self.validate_closure_counts()?;

        let expected_coverage = ExploreCoverage::from_counts(
            self.counts.admissible_configurations,
            self.counts.matching_configurations,
        );
        if self.coverage != expected_coverage {
            return Err(format!(
                "Explore coverage {:?} disagrees with count-derived coverage {:?}",
                self.coverage, expected_coverage
            ));
        }

        let search_decision_dag = self.validate_search_decision_dag_request()?;
        if let Some(search_decision_dag) = search_decision_dag {
            let multiplicities = SearchDecisionDagMultiplicities::from_graph(search_decision_dag)?;
            let open = multiplicities
                .eligibility_open
                .checked_add(multiplicities.polarity_open)
                .ok_or_else(|| {
                    "Explore search decision DAG open multiplicity exceeds u128::MAX".to_string()
                })?;
            if self.search.remaining_open_cases() != open {
                return Err(format!(
                    "Explore search reports {} open cases but its search decision DAG reports {open}",
                    self.search.remaining_open_cases()
                ));
            }
        }
        self.validate_results(search_decision_dag)?;
        self.validate_ledger(search_decision_dag)?;
        Ok(())
    }

    fn validate_declared_count(&self) -> Result<(), String> {
        let declared = self.schema.declared_assignment_count()?;
        if self.counts.declared_assignments != ExploreCount::Exact(declared) {
            return Err(format!(
                "Explore declared-assignment count {:?} disagrees with axis cardinality product {declared}",
                self.counts.declared_assignments
            ));
        }
        Ok(())
    }

    fn validate_closure_counts(&self) -> Result<(), String> {
        let exact_admissibility = self.counts.admissible_configurations.is_exact();
        if self.closures.admissibility.is_closed() != exact_admissibility {
            return Err(
                "Explore admissibility closure must agree with exact admissible count evidence"
                    .to_string(),
            );
        }

        let exact_polarity = self.counts.matching_configurations.is_exact();
        if self.closures.polarity.is_closed() != exact_polarity {
            return Err(
                "Explore polarity closure must agree with exact matching count evidence"
                    .to_string(),
            );
        }
        if self.closures.polarity.is_closed() && !self.closures.admissibility.is_closed() {
            return Err("Explore polarity cannot close before admissibility".to_string());
        }

        let exact_projection = self.counts.distinct_result_keys.is_exact();
        if self.closures.projection.is_closed() != exact_projection {
            return Err(
                "Explore projection closure must agree with exact result-key count evidence"
                    .to_string(),
            );
        }
        if self.closures.projection.is_closed() != self.group_counts.all_exact() {
            return Err(
                "Explore projection closure must agree with exact grouped-result accounting"
                    .to_string(),
            );
        }
        if self.closures.projection.is_closed() && !self.closures.polarity.is_closed() {
            return Err("Explore projection cannot close before polarity".to_string());
        }
        if self.closures.rows.is_closed() && !self.closures.representatives.is_closed() {
            return Err("Explore rows cannot close before representative selection".to_string());
        }
        Ok(())
    }

    fn validate_results(
        &self,
        search_decision_dag: Option<&CaseDecisionDag>,
    ) -> Result<(), String> {
        if !self.results.is_empty()
            && (!self.closures.representatives.is_closed() || !self.closures.rows.is_closed())
        {
            return Err(
                "Explore result rows require closed representative selection and row replay"
                    .to_string(),
            );
        }

        let mut representative_ids = BTreeSet::new();
        let mut support_sum = 0_u128;
        for row in &self.results {
            validate_case_id(&row.representative, &self.schema)?;
            if !representative_ids.insert(&row.representative) {
                return Err(
                    "one Explore case cannot represent more than one distinct result key"
                        .to_string(),
                );
            }
            validate_matching_case(
                search_decision_dag,
                &row.representative,
                "result representative",
            )?;
            if row.key.len() != self.schema.key_names.len() {
                return Err(format!(
                    "Explore result key has {} values for {} schema fields",
                    row.key.len(),
                    self.schema.key_names.len()
                ));
            }
            if row.extrema.len() != self.schema.extrema_names.len() {
                return Err(format!(
                    "Explore result row has {} extrema summaries for {} schema fields",
                    row.extrema.len(),
                    self.schema.extrema_names.len()
                ));
            }
            if row.shown.len() != self.schema.shown_names.len() {
                return Err(format!(
                    "Explore result row has {} shown values for {} schema fields",
                    row.shown.len(),
                    self.schema.shown_names.len()
                ));
            }
            if row.replay != ExploreReplayStatus::Confirmed {
                return Err("Explore result row is not replay-confirmed".to_string());
            }
            let support = row.support.proven_lower_bound().ok_or_else(|| {
                "Explore result row support must carry at least a confirmed lower bound".to_string()
            })?;
            if support == 0 {
                return Err("Explore result row support must be positive".to_string());
            }
            if !row.extrema.is_empty() && !self.closures.projection.is_closed() {
                return Err("Explore extrema rows require closed aggregate projection".to_string());
            }
            for summary in &row.extrema {
                validate_case_id(&summary.minimum_witness, &self.schema)?;
                validate_case_id(&summary.maximum_witness, &self.schema)?;
                validate_matching_case(
                    search_decision_dag,
                    &summary.minimum_witness,
                    "extrema minimum witness",
                )?;
                validate_matching_case(
                    search_decision_dag,
                    &summary.maximum_witness,
                    "extrema maximum witness",
                )?;
                if summary.minimum > summary.maximum {
                    return Err("Explore extrema minimum must not exceed its maximum".to_string());
                }
                let spread = (summary.maximum as i128 - summary.minimum as i128) as u128;
                if summary.spread != spread {
                    return Err(format!(
                        "Explore extrema spread {} disagrees with maximum - minimum {spread}",
                        summary.spread
                    ));
                }
                if summary.minimum_tie_support == 0 || summary.maximum_tie_support == 0 {
                    return Err("Explore extrema tie supports must be positive".to_string());
                }
                if summary.minimum_tie_support > support || summary.maximum_tie_support > support {
                    return Err(
                        "Explore extrema tie support exceeds result-key support".to_string()
                    );
                }
                if summary.spread == 0
                    && (summary.minimum_tie_support != support
                        || summary.maximum_tie_support != support)
                {
                    return Err(
                        "invariant Explore extrema must tie at both endpoints across the full key support"
                            .to_string(),
                    );
                }
                if summary.spread == 0 && summary.minimum_witness != summary.maximum_witness {
                    return Err(
                        "invariant Explore extrema must use one canonical endpoint witness"
                            .to_string(),
                    );
                }
                if summary.spread > 0 && summary.minimum_witness == summary.maximum_witness {
                    return Err(
                        "distinct Explore extrema endpoints require distinct witness cases"
                            .to_string(),
                    );
                }
                if summary.spread > 0
                    && summary
                        .minimum_tie_support
                        .checked_add(summary.maximum_tie_support)
                        .map_or(true, |ties| ties > support)
                {
                    return Err(
                        "distinct Explore extrema endpoint tie supports exceed result-key support"
                            .to_string(),
                    );
                }
            }
            if let ExploreGroupFilter::Varies { extrema_index } = self.schema.group_filter {
                if row.extrema[extrema_index].spread == 0 {
                    return Err(
                        "Explore varies filter emitted an invariant extrema group".to_string()
                    );
                }
            }
            if self.closures.projection.is_closed() != row.support.is_exact() {
                return Err(
                    "Explore result-row support is exact if and only if projection is closed"
                        .to_string(),
                );
            }
            support_sum = support_sum
                .checked_add(support)
                .ok_or_else(|| "Explore result-row support sum exceeds u128::MAX".to_string())?;
        }

        for pair in self.results.windows(2) {
            if pair[0].key >= pair[1].key {
                return Err(
                    "Explore result rows must have distinct keys in canonical order".to_string(),
                );
            }
        }

        if self.closures.representatives.is_closed() && self.closures.rows.is_closed() {
            if let Some(known) = self.counts.distinct_result_keys.proven_lower_bound() {
                if known != self.results.len() as u128 {
                    return Err(format!(
                        "Explore result row count {} disagrees with known distinct-key count {known}",
                        self.results.len()
                    ));
                }
            } else if !self.results.is_empty() {
                return Err(format!(
                    "Explore emitted {} result rows without a distinct-key count lower bound",
                    self.results.len(),
                ));
            }
        }
        if let Some(matching) = self.counts.matching_configurations.proven_lower_bound() {
            if support_sum > matching {
                return Err(format!(
                    "Explore result-row support sum {support_sum} exceeds matching count evidence {matching}"
                ));
            }
        }
        if let Some(qualifying) = self
            .group_counts
            .qualifying_configurations
            .proven_lower_bound()
        {
            if support_sum > qualifying {
                return Err(format!(
                    "Explore result-row support sum {support_sum} exceeds qualifying-configuration count evidence {qualifying}"
                ));
            }
        }
        if self.closures.polarity.is_closed()
            && self.closures.projection.is_closed()
            && self.closures.rows.is_closed()
        {
            let qualifying = self
                .group_counts
                .qualifying_configurations
                .exact()
                .expect("closed projection requires an exact qualifying count");
            if self.results.iter().any(|row| !row.support.is_exact()) {
                return Err(
                    "closed Explore projection requires exact support for every result row"
                        .to_string(),
                );
            }
            if support_sum != qualifying {
                return Err(format!(
                    "exact result-row support sum {support_sum} disagrees with qualifying-configuration count {qualifying}"
                ));
            }
        }
        Ok(())
    }

    fn validate_search_decision_dag_request(&self) -> Result<Option<&CaseDecisionDag>, String> {
        match (self.request.search_decision_dag, &self.search_decision_dag) {
            (ExploreSearchDecisionDagRequest::Omit, ExploreSearchDecisionDagEvidence::Omitted) => {
                Ok(None)
            }
            (
                ExploreSearchDecisionDagRequest::Include,
                ExploreSearchDecisionDagEvidence::Included(graph),
            ) => {
                self.validate_search_decision_dag(graph)?;
                Ok(Some(graph))
            }
            (
                ExploreSearchDecisionDagRequest::Omit,
                ExploreSearchDecisionDagEvidence::Included(_),
            ) => Err(
                "Explore report included a search decision DAG that the request omitted"
                    .to_string(),
            ),
            (
                ExploreSearchDecisionDagRequest::Include,
                ExploreSearchDecisionDagEvidence::Omitted,
            ) => Err("Explore report omitted its requested search decision DAG".to_string()),
        }
    }

    fn validate_search_decision_dag(&self, graph: &CaseDecisionDag) -> Result<(), String> {
        graph
            .validate()
            .map_err(|error| format!("Explore included search decision DAG is invalid: {error}"))?;
        if graph.axis_cardinalities() != self.schema.axis_cardinalities.as_ref() {
            return Err(format!(
                "Explore search decision DAG axis cardinalities {:?} disagree with report schema {:?}",
                graph.axis_cardinalities(),
                self.schema.axis_cardinalities
            ));
        }

        let multiplicities = SearchDecisionDagMultiplicities::from_graph(graph)?;
        multiplicities.validate_against(self.counts, self.closures)
    }

    fn validate_ledger(&self, search_decision_dag: Option<&CaseDecisionDag>) -> Result<(), String> {
        let rows = match (self.request.ledger, &self.ledger) {
            (ExploreLedgerRequest::Omit, ExploreLedgerEvidence::Omitted) => return Ok(()),
            (
                ExploreLedgerRequest::MatchingConfigurations,
                ExploreLedgerEvidence::MatchingConfigurations { rows },
            ) => rows,
            (ExploreLedgerRequest::Omit, ExploreLedgerEvidence::MatchingConfigurations { .. }) => {
                return Err(
                    "Explore report included a matching ledger that the request omitted"
                        .to_string(),
                )
            }
            (ExploreLedgerRequest::MatchingConfigurations, ExploreLedgerEvidence::Omitted) => {
                return Err("Explore report omitted its requested matching ledger".to_string())
            }
        };

        for row in rows {
            validate_case_id(&row.case_id, &self.schema)?;
            validate_matching_case(search_decision_dag, &row.case_id, "matching-ledger row")?;
            if row.dimensions.len() != self.schema.dimensions.len() {
                return Err(format!(
                    "Explore ledger row has {} dimension values for {} schema fields",
                    row.dimensions.len(),
                    self.schema.dimensions.len()
                ));
            }
            if row.key.len() != self.schema.key_names.len() {
                return Err(format!(
                    "Explore ledger key has {} values for {} schema fields",
                    row.key.len(),
                    self.schema.key_names.len()
                ));
            }
            if row.shown.len() != self.schema.shown_names.len() {
                return Err(format!(
                    "Explore ledger row has {} shown values for {} schema fields",
                    row.shown.len(),
                    self.schema.shown_names.len()
                ));
            }
            if row.replay != ExploreReplayStatus::Confirmed {
                return Err("Explore ledger row is not replay-confirmed".to_string());
            }
        }

        for pair in rows.windows(2) {
            if pair[0].case_id >= pair[1].case_id {
                return Err(
                    "Explore ledger rows must have distinct case IDs in canonical order"
                        .to_string(),
                );
            }
        }

        if self.closures.views.is_closed() {
            let exact = self.counts.matching_configurations.exact().ok_or_else(|| {
                "closed Explore matching ledger requires an exact matching count".to_string()
            })?;
            if exact != rows.len() as u128 {
                return Err(format!(
                    "Explore ledger row count {} disagrees with exact matching count {exact}",
                    rows.len()
                ));
            }
        }
        Ok(())
    }
}

fn validate_case_id(case_id: &ExploreCaseId, schema: &ExploreReportSchema) -> Result<(), String> {
    if case_id.len() != schema.dimensions.len() {
        return Err(format!(
            "Explore case ID has {} ordinals for {} declared dimensions",
            case_id.len(),
            schema.dimensions.len()
        ));
    }
    for (dimension, (&ordinal, &cardinality)) in case_id
        .ordinals()
        .iter()
        .zip(&schema.axis_cardinalities)
        .enumerate()
    {
        if ordinal >= cardinality {
            return Err(format!(
                "Explore case ID ordinal {ordinal} is outside dimension {dimension} with cardinality {cardinality}"
            ));
        }
    }
    Ok(())
}

fn validate_matching_case(
    graph: Option<&CaseDecisionDag>,
    case_id: &ExploreCaseId,
    kind: &str,
) -> Result<(), String> {
    let Some(graph) = graph else {
        // A replay-confirmed row remains independently valid when the request
        // deliberately omits the privacy-sensitive search decision DAG.
        return Ok(());
    };
    let terminal = graph
        .terminal_for_path(case_id.ordinals())
        .map_err(|error| format!("Explore {kind} has an invalid case ID: {error}"))?;
    if terminal != Some(&CaseTerminal::AdmissibleMatch) {
        return Err(format!(
            "Explore {kind} case {:?} is classified as {terminal:?}, expected AdmissibleMatch",
            case_id.ordinals()
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SearchDecisionDagMultiplicities {
    declared: u128,
    known_admissible: u128,
    matching: u128,
    eligibility_open: u128,
    polarity_open: u128,
}

impl SearchDecisionDagMultiplicities {
    fn from_graph(graph: &CaseDecisionDag) -> Result<Self, String> {
        let mut summary = Self {
            declared: 0,
            known_admissible: 0,
            matching: 0,
            eligibility_open: 0,
            polarity_open: 0,
        };
        for (terminal, cardinality) in graph.terminal_counts().map_err(|error| {
            format!("cannot count Explore search decision DAG terminals: {error}")
        })? {
            let count = match cardinality {
                CheckedCardinality::Exact(count) => count,
                CheckedCardinality::ExceedsU128 => {
                    return Err(format!(
                        "Explore search decision DAG terminal {terminal:?} multiplicity exceeds u128::MAX"
                    ))
                }
            };
            summary.declared = checked_graph_sum(summary.declared, count, "declared")?;
            match terminal {
                CaseTerminal::Excluded => {}
                CaseTerminal::EligibilityOpen(_) => {
                    summary.eligibility_open =
                        checked_graph_sum(summary.eligibility_open, count, "eligibility-open")?;
                }
                CaseTerminal::AdmissibleNonmatch => {
                    summary.known_admissible =
                        checked_graph_sum(summary.known_admissible, count, "known-admissible")?;
                }
                CaseTerminal::AdmissibleMatch => {
                    summary.known_admissible =
                        checked_graph_sum(summary.known_admissible, count, "known-admissible")?;
                    summary.matching = checked_graph_sum(summary.matching, count, "matching")?;
                }
                CaseTerminal::AdmissibleOpen(_) => {
                    summary.known_admissible =
                        checked_graph_sum(summary.known_admissible, count, "known-admissible")?;
                    summary.polarity_open =
                        checked_graph_sum(summary.polarity_open, count, "polarity-open")?;
                }
            }
        }
        Ok(summary)
    }

    fn validate_against(
        self,
        counts: ExploreCounts,
        closures: ExploreLayerClosures,
    ) -> Result<(), String> {
        require_graph_count(
            "declared assignments",
            counts.declared_assignments,
            ExploreCount::Exact(self.declared),
        )?;

        let admissibility_closed = self.eligibility_open == 0;
        let expected_admissible = if admissibility_closed {
            ExploreCount::Exact(self.known_admissible)
        } else {
            ExploreCount::LowerBound(self.known_admissible)
        };
        require_graph_count(
            "admissible configurations",
            counts.admissible_configurations,
            expected_admissible,
        )?;
        if closures.admissibility.is_closed() != admissibility_closed {
            return Err(format!(
                "Explore admissibility closure {:?} disagrees with search decision DAG eligibility-open multiplicity {}",
                closures.admissibility, self.eligibility_open
            ));
        }

        let polarity_closed = admissibility_closed && self.polarity_open == 0;
        let expected_matching = if polarity_closed {
            ExploreCount::Exact(self.matching)
        } else {
            ExploreCount::LowerBound(self.matching)
        };
        require_graph_count(
            "matching configurations",
            counts.matching_configurations,
            expected_matching,
        )?;
        if closures.polarity.is_closed() != polarity_closed {
            return Err(format!(
                "Explore polarity closure {:?} disagrees with search decision DAG open multiplicities (eligibility {}, polarity {})",
                closures.polarity, self.eligibility_open, self.polarity_open
            ));
        }
        Ok(())
    }
}

fn checked_graph_sum(left: u128, right: u128, kind: &str) -> Result<u128, String> {
    left.checked_add(right)
        .ok_or_else(|| format!("Explore search decision DAG {kind} multiplicity exceeds u128::MAX"))
}

fn require_graph_count(
    kind: &str,
    actual: ExploreCount,
    expected: ExploreCount,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "Explore {kind} count {actual:?} disagrees with search decision DAG evidence {expected:?}"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreStatus {
    Complete,
    Partial,
    Unknown,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreExactOutcome {
    Complete {
        method: ExploreCompletionMethod,
        evidence: ExploreExactEvidence,
    },
    Partial {
        stop: ExploreStopReason,
        evidence: ExploreExactEvidence,
    },
    Unknown {
        reason: String,
        evidence: ExploreExactEvidence,
    },
    Unsupported {
        diagnostic: String,
    },
    Error {
        diagnostics: Box<[String]>,
    },
}

impl ExploreExactOutcome {
    pub(crate) fn status(&self) -> ExploreStatus {
        match self {
            Self::Complete { .. } => ExploreStatus::Complete,
            Self::Partial { .. } => ExploreStatus::Partial,
            Self::Unknown { .. } => ExploreStatus::Unknown,
            Self::Unsupported { .. } => ExploreStatus::Unsupported,
            Self::Error { .. } => ExploreStatus::Error,
        }
    }

    pub(crate) fn evidence(&self) -> Option<&ExploreExactEvidence> {
        match self {
            Self::Complete { evidence, .. }
            | Self::Partial { evidence, .. }
            | Self::Unknown { evidence, .. } => Some(evidence),
            Self::Unsupported { .. } | Self::Error { .. } => None,
        }
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::Complete { method, evidence } => {
                evidence.validate()?;
                if !evidence.counts.all_exact() {
                    return Err("complete Explore report requires four exact counts".to_string());
                }
                if !evidence.closures.all_closed() {
                    return Err(
                        "complete Explore report requires every answer/case/value layer to close"
                            .to_string(),
                    );
                }
                if evidence.search.remaining_open_cases() != 0 {
                    return Err(
                        "complete Explore report requires exhausted search evidence".to_string()
                    );
                }
                if evidence.coverage == ExploreCoverage::Undetermined {
                    return Err(
                        "complete Explore report cannot have undetermined coverage".to_string()
                    );
                }
                method.validate_search(evidence.search)?;
                Ok(())
            }
            Self::Partial { stop, evidence } => {
                stop.validate()?;
                evidence.validate()?;
                if evidence.closures.all_closed() {
                    return Err(
                        "partial Explore report cannot carry fully closed evidence".to_string()
                    );
                }
                Ok(())
            }
            Self::Unknown { reason, evidence } => {
                if reason.is_empty() {
                    return Err("Explore unknown reason must not be empty".to_string());
                }
                evidence.validate()?;
                if evidence.closures.all_closed() {
                    return Err(
                        "unknown Explore report cannot carry fully closed evidence".to_string()
                    );
                }
                Ok(())
            }
            Self::Unsupported { diagnostic } => {
                if diagnostic.is_empty() {
                    Err("Explore unsupported diagnostic must not be empty".to_string())
                } else {
                    Ok(())
                }
            }
            Self::Error { diagnostics } => {
                if diagnostics.is_empty() || diagnostics.iter().any(String::is_empty) {
                    Err("Explore error requires nonempty diagnostics".to_string())
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// Terminal exact-finite report. Mechanism availability is orthogonal to the
/// answer outcome, so deferred tracing never downgrades a complete result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreExactReport {
    pub(crate) query_name: String,
    pub(crate) polarity: ExplorePolarity,
    pub(crate) mechanism: ExploreMechanismEvidence,
    pub(crate) outcome: ExploreExactOutcome,
}

impl ExploreExactReport {
    pub(crate) fn new(
        query_name: String,
        polarity: ExplorePolarity,
        mechanism: ExploreMechanismEvidence,
        outcome: ExploreExactOutcome,
    ) -> Result<Self, String> {
        let report = Self {
            query_name,
            polarity,
            mechanism,
            outcome,
        };
        report.validate()?;
        Ok(report)
    }

    pub(crate) fn with_deferred_mechanism(
        query_name: String,
        polarity: ExplorePolarity,
        outcome: ExploreExactOutcome,
    ) -> Result<Self, String> {
        Self::new(
            query_name,
            polarity,
            ExploreMechanismEvidence::deferred(),
            outcome,
        )
    }

    pub(crate) fn status(&self) -> ExploreStatus {
        self.outcome.status()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.query_name.is_empty() {
            return Err("Explore report query name must not be empty".to_string());
        }
        self.outcome.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::super::case_graph::{CaseGraphBuilder, CaseOpenReason};
    use super::*;

    fn schema(axis_cardinalities: Vec<u128>) -> ExploreReportSchema {
        ExploreReportSchema::new(
            axis_cardinalities
                .iter()
                .enumerate()
                .map(|(index, _)| ExploreReportDimension {
                    bound_index: index,
                    role: ExploreGeneratorAxisRole::Before,
                    role_field_index: index,
                    label: format!("axis_{index}"),
                })
                .collect::<Vec<_>>(),
            axis_cardinalities,
            vec!["kind".to_string()],
            Vec::<String>::new(),
        )
        .unwrap()
    }

    fn report(outcome: ExploreExactOutcome) -> Result<ExploreExactReport, String> {
        ExploreExactReport::with_deferred_mechanism(
            "example".to_string(),
            ExplorePolarity::Matches,
            outcome,
        )
    }

    #[test]
    fn equal_field_labels_in_different_transition_roles_are_distinct_axes() {
        let dimensions = vec![
            ExploreReportDimension {
                bound_index: 0,
                role: ExploreGeneratorAxisRole::Context,
                role_field_index: 0,
                label: "rate".to_string(),
            },
            ExploreReportDimension {
                bound_index: 1,
                role: ExploreGeneratorAxisRole::Before,
                role_field_index: 0,
                label: "rate".to_string(),
            },
        ];
        let schema = ExploreReportSchema::new(
            dimensions,
            vec![2, 3],
            vec!["result".to_string()],
            Vec::<String>::new(),
        )
        .expect("role-indexed dimensions must not collide by presentation label");

        assert_eq!(schema.dimensions[0].qualified_label(), "context.rate");
        assert_eq!(schema.dimensions[1].qualified_label(), "before.rate");
        assert_eq!(schema.declared_assignment_count(), Ok(6));
    }

    fn complete_evidence(
        schema: ExploreReportSchema,
        counts: ExploreCounts,
        coverage: ExploreCoverage,
        results: Vec<ExploreResultRow>,
        search_decision_dag: ExploreSearchDecisionDagEvidence,
    ) -> ExploreExactEvidence {
        let declared = schema.declared_assignment_count().unwrap();
        ExploreExactEvidence {
            request: ExploreReportRequest {
                search_decision_dag: match &search_decision_dag {
                    ExploreSearchDecisionDagEvidence::Omitted => {
                        ExploreSearchDecisionDagRequest::Omit
                    }
                    ExploreSearchDecisionDagEvidence::Included(_) => {
                        ExploreSearchDecisionDagRequest::Include
                    }
                },
                semantic_transition_graph: ExploreSemanticTransitionGraphRequest::Omit,
                ledger: ExploreLedgerRequest::Omit,
            },
            schema,
            search: ExploreSearchEvidence::Canonical {
                classified_cases: declared,
                remaining_open_cases: 0,
                exhausted: true,
            },
            counts,
            group_counts: ExploreGroupCounts::unfiltered(
                counts.distinct_result_keys,
                counts.matching_configurations,
            ),
            coverage,
            closures: ExploreLayerClosures::closed(),
            results: results.into_boxed_slice(),
            search_decision_dag,
            ledger: ExploreLedgerEvidence::Omitted,
        }
    }

    #[test]
    fn complete_empty_space_remains_a_valid_exact_result() {
        let graph = CaseGraphBuilder::new(vec![0]).finish_complete().unwrap();
        let evidence = complete_evidence(
            schema(vec![0]),
            ExploreCounts {
                declared_assignments: ExploreCount::Exact(0),
                admissible_configurations: ExploreCount::Exact(0),
                matching_configurations: ExploreCount::Exact(0),
                distinct_result_keys: ExploreCount::Exact(0),
            },
            ExploreCoverage::Empty,
            Vec::new(),
            ExploreSearchDecisionDagEvidence::Included(graph),
        );
        report(ExploreExactOutcome::Complete {
            method: ExploreCompletionMethod::ExactFiniteExhaustion,
            evidence,
        })
        .unwrap();
    }

    #[test]
    fn case_ids_must_fit_every_axis_cardinality() {
        let evidence = complete_evidence(
            schema(vec![2]),
            ExploreCounts {
                declared_assignments: ExploreCount::Exact(2),
                admissible_configurations: ExploreCount::Exact(1),
                matching_configurations: ExploreCount::Exact(1),
                distinct_result_keys: ExploreCount::Exact(1),
            },
            ExploreCoverage::All,
            vec![ExploreResultRow::confirmed(
                ExploreResultKey::new(vec![ExploreValue::String("cliff".to_string())]),
                Vec::<ExploreValue>::new(),
                ExploreCaseId::new(vec![2]),
            )],
            ExploreSearchDecisionDagEvidence::Omitted,
        );
        let error = report(ExploreExactOutcome::Complete {
            method: ExploreCompletionMethod::ExactFiniteExhaustion,
            evidence,
        })
        .unwrap_err();
        assert!(error.contains("ordinal 2 is outside dimension 0"));
    }

    #[test]
    fn included_graph_must_classify_representatives_as_matches() {
        let mut builder = CaseGraphBuilder::new(vec![2]);
        builder.push_next(CaseTerminal::AdmissibleMatch).unwrap();
        builder.push_next(CaseTerminal::AdmissibleNonmatch).unwrap();
        let evidence = complete_evidence(
            schema(vec![2]),
            ExploreCounts {
                declared_assignments: ExploreCount::Exact(2),
                admissible_configurations: ExploreCount::Exact(2),
                matching_configurations: ExploreCount::Exact(1),
                distinct_result_keys: ExploreCount::Exact(1),
            },
            ExploreCoverage::Some,
            vec![ExploreResultRow::confirmed(
                ExploreResultKey::new(vec![ExploreValue::String("cliff".to_string())]),
                Vec::<ExploreValue>::new(),
                ExploreCaseId::new(vec![1]),
            )],
            ExploreSearchDecisionDagEvidence::Included(builder.finish_complete().unwrap()),
        );
        let error = report(ExploreExactOutcome::Complete {
            method: ExploreCompletionMethod::ExactFiniteExhaustion,
            evidence,
        })
        .unwrap_err();
        assert!(error.contains("expected AdmissibleMatch"));
    }

    #[test]
    fn included_graph_multiplicities_are_report_count_evidence() {
        let mut builder = CaseGraphBuilder::new(vec![2]);
        builder.push_next(CaseTerminal::AdmissibleMatch).unwrap();
        builder.push_next(CaseTerminal::Excluded).unwrap();
        let evidence = complete_evidence(
            schema(vec![2]),
            ExploreCounts {
                declared_assignments: ExploreCount::Exact(2),
                admissible_configurations: ExploreCount::Exact(2),
                matching_configurations: ExploreCount::Exact(1),
                distinct_result_keys: ExploreCount::Exact(0),
            },
            ExploreCoverage::Some,
            Vec::new(),
            ExploreSearchDecisionDagEvidence::Included(builder.finish_complete().unwrap()),
        );
        let error = report(ExploreExactOutcome::Complete {
            method: ExploreCompletionMethod::ExactFiniteExhaustion,
            evidence,
        })
        .unwrap_err();
        assert!(error.contains("search decision DAG evidence"));
    }

    #[test]
    fn included_graph_must_classify_ledger_ids_as_matches() {
        let mut builder = CaseGraphBuilder::new(vec![2]);
        builder.push_next(CaseTerminal::AdmissibleMatch).unwrap();
        builder.push_next(CaseTerminal::AdmissibleNonmatch).unwrap();
        let evidence = ExploreExactEvidence {
            request: ExploreReportRequest {
                search_decision_dag: ExploreSearchDecisionDagRequest::Include,
                semantic_transition_graph: ExploreSemanticTransitionGraphRequest::Omit,
                ledger: ExploreLedgerRequest::MatchingConfigurations,
            },
            schema: schema(vec![2]),
            search: ExploreSearchEvidence::Canonical {
                classified_cases: 2,
                remaining_open_cases: 0,
                exhausted: true,
            },
            counts: ExploreCounts {
                declared_assignments: ExploreCount::Exact(2),
                admissible_configurations: ExploreCount::Exact(2),
                matching_configurations: ExploreCount::Exact(1),
                distinct_result_keys: ExploreCount::Exact(1),
            },
            group_counts: ExploreGroupCounts::unfiltered(
                ExploreCount::Exact(1),
                ExploreCount::Exact(1),
            ),
            coverage: ExploreCoverage::Some,
            closures: ExploreLayerClosures::closed(),
            results: vec![ExploreResultRow::confirmed(
                ExploreResultKey::new(vec![ExploreValue::String("cliff".to_string())]),
                Vec::<ExploreValue>::new(),
                ExploreCaseId::new(vec![0]),
            )]
            .into_boxed_slice(),
            search_decision_dag: ExploreSearchDecisionDagEvidence::Included(
                builder.finish_complete().unwrap(),
            ),
            ledger: ExploreLedgerEvidence::MatchingConfigurations {
                rows: vec![ExploreLedgerRow::confirmed(
                    ExploreCaseId::new(vec![1]),
                    vec![ExploreValue::Int(20)],
                    ExploreResultKey::new(vec![ExploreValue::String("cliff".to_string())]),
                    Vec::<ExploreValue>::new(),
                )]
                .into_boxed_slice(),
            },
        };
        let error = report(ExploreExactOutcome::Complete {
            method: ExploreCompletionMethod::ExactFiniteExhaustion,
            evidence,
        })
        .unwrap_err();
        assert!(error.contains("matching-ledger row"));
        assert!(error.contains("expected AdmissibleMatch"));
    }

    #[test]
    fn included_open_frontier_proves_exact_lower_bounds() {
        let mut builder = CaseGraphBuilder::new(vec![2]);
        builder.push_next(CaseTerminal::AdmissibleMatch).unwrap();
        let graph = builder
            .finish_with_remainder(CaseTerminal::EligibilityOpen(
                CaseOpenReason::SearchBudgetExhausted,
            ))
            .unwrap();
        let evidence = ExploreExactEvidence {
            request: ExploreReportRequest {
                search_decision_dag: ExploreSearchDecisionDagRequest::Include,
                semantic_transition_graph: ExploreSemanticTransitionGraphRequest::Omit,
                ledger: ExploreLedgerRequest::Omit,
            },
            schema: schema(vec![2]),
            search: ExploreSearchEvidence::Canonical {
                classified_cases: 1,
                remaining_open_cases: 1,
                exhausted: false,
            },
            counts: ExploreCounts {
                declared_assignments: ExploreCount::Exact(2),
                admissible_configurations: ExploreCount::LowerBound(1),
                matching_configurations: ExploreCount::LowerBound(1),
                distinct_result_keys: ExploreCount::LowerBound(1),
            },
            group_counts: ExploreGroupCounts::unfiltered(
                ExploreCount::LowerBound(1),
                ExploreCount::LowerBound(1),
            ),
            coverage: ExploreCoverage::Undetermined,
            closures: ExploreLayerClosures {
                admissibility: ExploreClosure::Open,
                polarity: ExploreClosure::Open,
                projection: ExploreClosure::Open,
                representatives: ExploreClosure::Closed,
                rows: ExploreClosure::Closed,
                views: ExploreClosure::Closed,
            },
            results: vec![ExploreResultRow::confirmed_with_support(
                ExploreResultKey::new(vec![ExploreValue::String("cliff".to_string())]),
                Vec::<ExploreValue>::new(),
                ExploreCaseId::new(vec![0]),
                ExploreCount::LowerBound(1),
            )]
            .into_boxed_slice(),
            search_decision_dag: ExploreSearchDecisionDagEvidence::Included(graph),
            ledger: ExploreLedgerEvidence::Omitted,
        };
        report(ExploreExactOutcome::Partial {
            stop: ExploreStopReason::CaseLimit { limit: 1 },
            evidence,
        })
        .unwrap();
    }

    #[test]
    fn partial_outcome_requires_at_least_one_open_layer() {
        let evidence = complete_evidence(
            schema(vec![1]),
            ExploreCounts {
                declared_assignments: ExploreCount::Exact(1),
                admissible_configurations: ExploreCount::Exact(1),
                matching_configurations: ExploreCount::Exact(0),
                distinct_result_keys: ExploreCount::Exact(0),
            },
            ExploreCoverage::None,
            Vec::new(),
            ExploreSearchDecisionDagEvidence::Omitted,
        );
        let error = report(ExploreExactOutcome::Partial {
            stop: ExploreStopReason::CaseLimit { limit: 1 },
            evidence,
        })
        .unwrap_err();
        assert!(error.contains("fully closed evidence"));
    }

    #[test]
    fn unknown_outcome_requires_at_least_one_open_layer() {
        let evidence = complete_evidence(
            schema(vec![1]),
            ExploreCounts {
                declared_assignments: ExploreCount::Exact(1),
                admissible_configurations: ExploreCount::Exact(1),
                matching_configurations: ExploreCount::Exact(0),
                distinct_result_keys: ExploreCount::Exact(0),
            },
            ExploreCoverage::None,
            Vec::new(),
            ExploreSearchDecisionDagEvidence::Omitted,
        );
        let error = report(ExploreExactOutcome::Unknown {
            reason: "solver returned unknown".to_string(),
            evidence,
        })
        .unwrap_err();
        assert!(error.contains("fully closed evidence"));
    }

    #[test]
    fn open_row_replay_cannot_emit_a_result_prefix() {
        let evidence = ExploreExactEvidence {
            request: ExploreReportRequest::baseline(),
            schema: schema(vec![2]),
            search: ExploreSearchEvidence::Canonical {
                classified_cases: 1,
                remaining_open_cases: 1,
                exhausted: false,
            },
            counts: ExploreCounts {
                declared_assignments: ExploreCount::Exact(2),
                admissible_configurations: ExploreCount::LowerBound(1),
                matching_configurations: ExploreCount::LowerBound(1),
                distinct_result_keys: ExploreCount::LowerBound(1),
            },
            group_counts: ExploreGroupCounts::unfiltered(
                ExploreCount::LowerBound(1),
                ExploreCount::LowerBound(1),
            ),
            coverage: ExploreCoverage::Undetermined,
            closures: ExploreLayerClosures {
                admissibility: ExploreClosure::Open,
                polarity: ExploreClosure::Open,
                projection: ExploreClosure::Open,
                representatives: ExploreClosure::Closed,
                rows: ExploreClosure::Open,
                views: ExploreClosure::Closed,
            },
            results: vec![ExploreResultRow::confirmed(
                ExploreResultKey::new(vec![ExploreValue::String("cliff".to_string())]),
                Vec::<ExploreValue>::new(),
                ExploreCaseId::new(vec![0]),
            )]
            .into_boxed_slice(),
            search_decision_dag: ExploreSearchDecisionDagEvidence::Omitted,
            ledger: ExploreLedgerEvidence::Omitted,
        };
        let error = report(ExploreExactOutcome::Partial {
            stop: ExploreStopReason::CaseLimit { limit: 1 },
            evidence,
        })
        .unwrap_err();
        assert!(error.contains("closed representative selection and row replay"));
    }

    #[test]
    fn source_candidate_search_accounts_for_pending_open_work() {
        ExploreSearchEvidence::SourceCandidateFirst {
            distinct_source_candidates: 2,
            scheduled_source_candidates: 2,
            evaluated_source_candidates: 1,
            scheduled_fallback_cases: 0,
            evaluated_fallback_cases: 0,
            singleton_closed_cases: 1,
            certified_region_closed_cases: 0,
            pending_evaluations: 1,
            remaining_open_cases: 2,
            exhausted: false,
        }
        .validate(3)
        .unwrap();
    }

    #[test]
    fn source_candidate_search_cannot_be_exhausted_with_pending_work() {
        let error = ExploreSearchEvidence::SourceCandidateFirst {
            distinct_source_candidates: 1,
            scheduled_source_candidates: 1,
            evaluated_source_candidates: 0,
            scheduled_fallback_cases: 0,
            evaluated_fallback_cases: 0,
            singleton_closed_cases: 0,
            certified_region_closed_cases: 1,
            pending_evaluations: 1,
            remaining_open_cases: 0,
            exhausted: true,
        }
        .validate(1)
        .unwrap_err();
        assert!(error.contains("pending work exceeds remaining open cases"));
    }
}
