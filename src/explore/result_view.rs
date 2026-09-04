//! Pure relational reduction for evaluated Explore result-view contributions.
//!
//! Expression evaluation is deliberately outside this module. A runtime
//! adapter supplies canonical typed values for one stable case or mechanism
//! incidence row and evaluates the checked public projection when the reducer
//! presents a closed row/group environment. This reducer owns set semantics,
//! grouping, closed aggregates, deterministic choice, closure-aware status,
//! privacy projection, and canonical snapshots.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::mechanism_incidence::MechanismSignatureId;
use super::relation::{RelationalCaseId, SourceKey, ViewId};
use super::structural_mechanism::{ExecutionProfileId, StructuralMechanismId};
use super::transition::{canonical_explore_value_digest, TransitionId};
use super::ExploreValue;
use crate::{ExploreChooseCardinality, ExploreOptimizeDirection};

const RESULT_VIEW_SPEC_ROOT_HASH_V1: &[u8] =
    b"futuruna.explore.relational-result-view-spec-root.v1";
const RESULT_VIEW_ROOT_HASH_V3: &[u8] = b"futuruna.explore.relational-result-view-root.v3";
const CERTIFIED_RESULT_INPUT_ROOT_V1: &[u8] = b"futuruna.explore.certified-result-input-root.v1";
const CERTIFIED_RESULT_VIEW_ROOT_V1: &[u8] = b"futuruna.explore.certified-result-view-root.v1";

/// Canonical commitment to every semantic field of one lowered result-view
/// reducer contract. Display addresses and declaration positions never enter
/// this root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResultViewSpecRoot([u8; 32]);

impl ResultViewSpecRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Typed identity kind of the relation consumed by one result view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewInputKind {
    Source,
    Case,
    Incidence,
}

/// Exact identity of one row in a mechanism request's incidence relation.
///
/// The case remains present even when several cases share a signature. The
/// transition remains present even when the same signature spans disconnected
/// parts of the explored relation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MechanismIncidenceRowId {
    case_id: RelationalCaseId,
    transition_id: TransitionId,
    signature_id: MechanismSignatureId,
}

impl MechanismIncidenceRowId {
    pub(crate) const fn new(
        case_id: RelationalCaseId,
        transition_id: TransitionId,
        signature_id: MechanismSignatureId,
    ) -> Self {
        Self {
            case_id,
            transition_id,
            signature_id,
        }
    }

    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) const fn transition_id(self) -> TransitionId {
        self.transition_id
    }

    pub(crate) const fn signature_id(self) -> MechanismSignatureId {
        self.signature_id
    }
}

/// Stable identity of one row presented to an evaluated result view.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ResultViewInputRowId {
    Source(SourceKey),
    Case(RelationalCaseId),
    Incidence(MechanismIncidenceRowId),
}

impl ResultViewInputRowId {
    pub(crate) const fn kind(self) -> ResultViewInputKind {
        match self {
            Self::Source(_) => ResultViewInputKind::Source,
            Self::Case(_) => ResultViewInputKind::Case,
            Self::Incidence(_) => ResultViewInputKind::Incidence,
        }
    }

    pub(crate) const fn case_id(self) -> Option<RelationalCaseId> {
        match self {
            Self::Source(_) => None,
            Self::Case(case_id) => Some(case_id),
            Self::Incidence(incidence) => Some(incidence.case_id),
        }
    }
}

/// Canonical value admitted at the evaluated reducer boundary.
///
/// Semantic IDs are not stringified. They retain their distinct types for
/// grouping, `count_distinct`, projections, and authenticated replay.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ResultValue {
    Value(ExploreValue),
    CaseId(RelationalCaseId),
    TransitionId(TransitionId),
    SignatureId(MechanismSignatureId),
    StructuralMechanismId(StructuralMechanismId),
    ExecutionProfileId(ExecutionProfileId),
}

impl From<ExploreValue> for ResultValue {
    fn from(value: ExploreValue) -> Self {
        Self::Value(value)
    }
}

impl From<RelationalCaseId> for ResultValue {
    fn from(value: RelationalCaseId) -> Self {
        Self::CaseId(value)
    }
}

impl From<TransitionId> for ResultValue {
    fn from(value: TransitionId) -> Self {
        Self::TransitionId(value)
    }
}

impl From<MechanismSignatureId> for ResultValue {
    fn from(value: MechanismSignatureId) -> Self {
        Self::SignatureId(value)
    }
}

impl From<StructuralMechanismId> for ResultValue {
    fn from(value: StructuralMechanismId) -> Self {
        Self::StructuralMechanismId(value)
    }
}

impl From<ExecutionProfileId> for ResultValue {
    fn from(value: ExecutionProfileId) -> Self {
        Self::ExecutionProfileId(value)
    }
}

impl ResultValue {
    pub(crate) fn canonicalize_value_storage(
        &mut self,
        visitor: &mut impl FnMut(&mut ExploreValue),
    ) {
        if let Self::Value(value) = self {
            visitor(value);
        }
    }
}

/// Evaluated grain. Group keys are ordinary named typed values; an interval or
/// bucket has no privileged reducer representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewGrain {
    EachCase,
    EachIncidence,
    GroupAll,
    GroupBy { field_names: Box<[Box<str>]> },
}

impl ResultViewGrain {
    pub(crate) const fn is_grouped(&self) -> bool {
        matches!(self, Self::GroupAll | Self::GroupBy { .. })
    }

    pub(crate) fn group_field_names(&self) -> &[Box<str>] {
        match self {
            Self::GroupBy { field_names } => field_names,
            Self::EachCase | Self::EachIncidence | Self::GroupAll => &[],
        }
    }

    pub(crate) fn group_value_count(&self) -> usize {
        self.group_field_names().len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewHaving {
    Varies { measure_index: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewChoice {
    Optimize {
        cardinality: ExploreChooseCardinality,
        direction: ExploreOptimizeDirection,
    },
    Pareto {
        directions: Box<[ExploreOptimizeDirection]>,
    },
}

impl ResultViewChoice {
    pub(crate) fn objective_count(&self) -> usize {
        match self {
            Self::Optimize { .. } => 1,
            Self::Pareto { directions } => directions.len(),
        }
    }
}

/// Closed reducer schema minted together with one [`ViewId`].
///
/// Names live here once, not in every contribution. Value arrays follow these
/// canonical declaration orders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultViewSpec {
    spec_root: ResultViewSpecRoot,
    view_id: ViewId,
    input_kind: ResultViewInputKind,
    grain: ResultViewGrain,
    measure_names: Box<[Box<str>]>,
    aggregate_names: Box<[Box<str>]>,
    projection_names: Box<[Box<str>]>,
    having: Option<ResultViewHaving>,
    choice: Option<ResultViewChoice>,
}

impl ResultViewSpec {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        view_id: ViewId,
        input_kind: ResultViewInputKind,
        grain: ResultViewGrain,
        measure_names: Box<[Box<str>]>,
        aggregate_names: Box<[Box<str>]>,
        projection_names: Box<[Box<str>]>,
        having: Option<ResultViewHaving>,
        choice: Option<ResultViewChoice>,
    ) -> Result<Self, ResultViewError> {
        match (&grain, input_kind) {
            (ResultViewGrain::EachCase, ResultViewInputKind::Case)
            | (ResultViewGrain::EachIncidence, ResultViewInputKind::Incidence)
            | (ResultViewGrain::GroupAll, _)
            | (ResultViewGrain::GroupBy { .. }, _) => {}
            _ => return Err(ResultViewError::InputGrainMismatch),
        }
        if matches!(
            &grain,
            ResultViewGrain::GroupBy { field_names } if field_names.is_empty()
        ) {
            return Err(ResultViewError::EmptyGroupBy);
        }
        if !grain.is_grouped() && !aggregate_names.is_empty() {
            return Err(ResultViewError::AggregateRequiresGroupedGrain);
        }
        if !grain.is_grouped() && having.is_some() {
            return Err(ResultViewError::HavingRequiresGroupedGrain);
        }
        if let Some(ResultViewHaving::Varies { measure_index }) = having {
            if measure_index >= measure_names.len() {
                return Err(ResultViewError::UnknownHavingMeasure { measure_index });
            }
        }
        if matches!(
            &choice,
            Some(ResultViewChoice::Pareto { directions }) if directions.is_empty()
        ) {
            return Err(ResultViewError::EmptyParetoObjectives);
        }

        let mut intermediate_names = BTreeSet::<&str>::new();
        for name in grain
            .group_field_names()
            .iter()
            .chain(measure_names.iter())
            .chain(aggregate_names.iter())
        {
            if name.is_empty() || !intermediate_names.insert(name.as_ref()) {
                return Err(ResultViewError::InvalidIntermediateNames);
            }
        }
        let mut projected_names = BTreeSet::<&str>::new();
        for name in projection_names.iter() {
            if name.is_empty() || !projected_names.insert(name.as_ref()) {
                return Err(ResultViewError::InvalidProjectionNames);
            }
        }

        let mut spec = Self {
            spec_root: ResultViewSpecRoot([0; 32]),
            view_id,
            input_kind,
            grain,
            measure_names,
            aggregate_names,
            projection_names,
            having,
            choice,
        };
        spec.spec_root = derive_result_view_spec_root(&spec);
        Ok(spec)
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) fn validate_spec_root(&self) -> Result<(), ResultViewError> {
        if derive_result_view_spec_root(self) != self.spec_root {
            return Err(ResultViewError::SpecRootMismatch);
        }
        Ok(())
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn input_kind(&self) -> ResultViewInputKind {
        self.input_kind
    }

    pub(crate) const fn grain(&self) -> &ResultViewGrain {
        &self.grain
    }

    pub(crate) fn measure_names(&self) -> &[Box<str>] {
        &self.measure_names
    }

    pub(crate) fn aggregate_names(&self) -> &[Box<str>] {
        &self.aggregate_names
    }

    pub(crate) fn projection_names(&self) -> &[Box<str>] {
        &self.projection_names
    }

    pub(crate) const fn having(&self) -> Option<ResultViewHaving> {
        self.having
    }

    pub(crate) fn choice(&self) -> Option<&ResultViewChoice> {
        self.choice.as_ref()
    }

    /// Grouped views without a row choice can close from canonical durable
    /// contributions without retaining relation-owned row bindings.
    pub(crate) fn supports_borrowed_group_close(&self) -> bool {
        self.grain.is_grouped() && self.choice.is_none()
    }

    fn objective_count(&self) -> usize {
        self.choice
            .as_ref()
            .map_or(0, ResultViewChoice::objective_count)
    }
}

/// One fully evaluated contribution for one exact input-relation row. The
/// reducer validates every array against the named schema before accepting it.
/// A support cell with cardinality greater than one requires a separate
/// weighted/certified contribution API and must not borrow one representative
/// row identity here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvaluatedResultContribution {
    view_id: ViewId,
    row_id: ResultViewInputRowId,
    group_values: Box<[ResultValue]>,
    measures: Box<[ResultValue]>,
    distinct_arguments: Box<[ResultValue]>,
}

impl EvaluatedResultContribution {
    pub(crate) fn new(
        view_id: ViewId,
        row_id: ResultViewInputRowId,
        group_values: impl Into<Box<[ResultValue]>>,
        measures: impl Into<Box<[ResultValue]>>,
        distinct_arguments: impl Into<Box<[ResultValue]>>,
    ) -> Self {
        Self {
            view_id,
            row_id,
            group_values: group_values.into(),
            measures: measures.into(),
            distinct_arguments: distinct_arguments.into(),
        }
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn row_id(&self) -> ResultViewInputRowId {
        self.row_id
    }

    pub(crate) fn group_values(&self) -> &[ResultValue] {
        &self.group_values
    }

    pub(crate) fn measures(&self) -> &[ResultValue] {
        &self.measures
    }

    pub(crate) fn distinct_arguments(&self) -> &[ResultValue] {
        &self.distinct_arguments
    }

    pub(crate) fn canonicalize_value_storage(
        &mut self,
        visitor: &mut impl FnMut(&mut ExploreValue),
    ) {
        for value in self
            .group_values
            .iter_mut()
            .chain(self.measures.iter_mut())
            .chain(self.distinct_arguments.iter_mut())
        {
            value.canonicalize_value_storage(visitor);
        }
    }
}

/// Reducer-owned, group-closed environment presented to the checked
/// expression adapter. Aggregate counts are lower bounds while the input is
/// open and exact after it is sealed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResultClosedGroupRef<'a> {
    key: &'a ResultGroupKey,
    aggregates: &'a [ResultCountDistinctSnapshot],
    input_sealed: bool,
}

impl<'a> ResultClosedGroupRef<'a> {
    pub(crate) const fn key(self) -> &'a ResultGroupKey {
        self.key
    }

    pub(crate) const fn aggregates(self) -> &'a [ResultCountDistinctSnapshot] {
        self.aggregates
    }

    pub(crate) const fn input_is_sealed(self) -> bool {
        self.input_sealed
    }
}

/// One exact candidate row plus its optional group-closed environment.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResultClosedRowRef<'a> {
    contribution: &'a EvaluatedResultContribution,
    group: Option<ResultClosedGroupRef<'a>>,
}

impl<'a> ResultClosedRowRef<'a> {
    pub(crate) const fn contribution(self) -> &'a EvaluatedResultContribution {
        self.contribution
    }

    pub(crate) const fn group(self) -> Option<ResultClosedGroupRef<'a>> {
        self.group
    }
}

/// Expression-independent boundary between closed reduction and the checked
/// result-expression runtime.
pub(crate) trait ResultViewProjector {
    fn project_group(
        &mut self,
        group: ResultClosedGroupRef<'_>,
    ) -> Result<Box<[ResultValue]>, ResultViewProjectionError>;

    /// Evaluate only the choice objective vector needed to close a choice.
    /// The reducer retains row identity, so equal vectors remain distinct
    /// candidates.
    fn evaluate_objectives(
        &mut self,
        row: ResultClosedRowRef<'_>,
    ) -> Result<Box<[i64]>, ResultViewProjectionError>;

    /// Evaluate the public SELECT projection only for a row the reducer has
    /// decided to publish (or every row when no choice is declared).
    fn project_row(
        &mut self,
        row: ResultClosedRowRef<'_>,
    ) -> Result<Box<[ResultValue]>, ResultViewProjectionError>;
}

/// Count whose interpretation remains honest while a view is still open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewCount {
    LowerBound(u128),
    Provisional(u128),
    Exact(u128),
}

impl ResultViewCount {
    pub(crate) const fn current(self) -> u128 {
        match self {
            Self::LowerBound(value) | Self::Provisional(value) | Self::Exact(value) => value,
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewStatus {
    Provisional,
    Exact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResultViewCounts {
    input_rows: ResultViewCount,
    groups: Option<ResultViewCount>,
    output_groups: Option<ResultViewCount>,
    output_rows: ResultViewCount,
}

impl ResultViewCounts {
    pub(super) const fn from_journal_codec_parts(
        input_rows: ResultViewCount,
        groups: Option<ResultViewCount>,
        output_groups: Option<ResultViewCount>,
        output_rows: ResultViewCount,
    ) -> Self {
        Self {
            input_rows,
            groups,
            output_groups,
            output_rows,
        }
    }

    pub(crate) const fn input_rows(self) -> ResultViewCount {
        self.input_rows
    }

    pub(crate) const fn groups(self) -> Option<ResultViewCount> {
        self.groups
    }

    pub(crate) const fn output_groups(self) -> Option<ResultViewCount> {
        self.output_groups
    }

    pub(crate) const fn output_rows(self) -> ResultViewCount {
        self.output_rows
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResultGroupKey(Box<[ResultValue]>);

impl ResultGroupKey {
    pub(super) fn from_journal_codec_values(values: Box<[ResultValue]>) -> Self {
        Self(values)
    }

    fn new(values: Box<[ResultValue]>) -> Self {
        Self(values)
    }

    pub(crate) fn values(&self) -> &[ResultValue] {
        &self.0
    }

    pub(crate) fn canonicalize_value_storage(
        &mut self,
        visitor: &mut impl FnMut(&mut super::ExploreValue),
    ) {
        for value in self.0.iter_mut() {
            value.canonicalize_value_storage(visitor);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultGroupDisposition {
    Provisional { currently_passes_having: bool },
    ExactIncluded,
    ExactExcluded,
}

impl ResultGroupDisposition {
    pub(crate) const fn currently_passes(self) -> bool {
        match self {
            Self::Provisional {
                currently_passes_having,
            } => currently_passes_having,
            Self::ExactIncluded => true,
            Self::ExactExcluded => false,
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::ExactIncluded | Self::ExactExcluded)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultCountDistinctSnapshot {
    name: Box<str>,
    count: ResultViewCount,
}

impl ResultCountDistinctSnapshot {
    pub(super) fn from_journal_codec_parts(name: Box<str>, count: ResultViewCount) -> Self {
        Self { name, count }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn count(&self) -> ResultViewCount {
        self.count
    }
}

/// One selected or provisionally selected input row. Row identity is retained
/// independently of equal projected values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultOutputRow {
    row_id: ResultViewInputRowId,
    values: Box<[ResultValue]>,
}

impl ResultOutputRow {
    pub(super) fn from_journal_codec_parts(
        row_id: ResultViewInputRowId,
        values: Box<[ResultValue]>,
    ) -> Self {
        Self { row_id, values }
    }

    pub(crate) const fn row_id(&self) -> ResultViewInputRowId {
        self.row_id
    }

    pub(crate) fn values(&self) -> &[ResultValue] {
        &self.values
    }

    pub(crate) fn canonicalize_value_storage(
        &mut self,
        visitor: &mut impl FnMut(&mut super::ExploreValue),
    ) {
        for value in self.values.iter_mut() {
            value.canonicalize_value_storage(visitor);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultGroupSnapshot {
    key: ResultGroupKey,
    member_count: ResultViewCount,
    observed_having_varies: Option<bool>,
    disposition: ResultGroupDisposition,
    aggregates: Box<[ResultCountDistinctSnapshot]>,
    projected_values: Option<Box<[ResultValue]>>,
    chosen_rows: Box<[ResultOutputRow]>,
}

impl ResultGroupSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_journal_codec_parts(
        key: ResultGroupKey,
        member_count: ResultViewCount,
        observed_having_varies: Option<bool>,
        disposition: ResultGroupDisposition,
        aggregates: Box<[ResultCountDistinctSnapshot]>,
        projected_values: Option<Box<[ResultValue]>>,
        chosen_rows: Box<[ResultOutputRow]>,
    ) -> Self {
        Self {
            key,
            member_count,
            observed_having_varies,
            disposition,
            aggregates,
            projected_values,
            chosen_rows,
        }
    }

    pub(crate) const fn key(&self) -> &ResultGroupKey {
        &self.key
    }

    pub(crate) fn aggregates(&self) -> &[ResultCountDistinctSnapshot] {
        &self.aggregates
    }

    pub(crate) const fn member_count(&self) -> ResultViewCount {
        self.member_count
    }

    pub(crate) const fn observed_having_varies(&self) -> Option<bool> {
        self.observed_having_varies
    }

    pub(crate) const fn disposition(&self) -> ResultGroupDisposition {
        self.disposition
    }

    /// Public SELECT values for a grouped view without choice. Diagnostic
    /// group keys and aggregate evidence remain separate from this projection.
    pub(crate) fn projected_values(&self) -> Option<&[ResultValue]> {
        self.projected_values.as_deref()
    }

    pub(crate) fn chosen_rows(&self) -> &[ResultOutputRow] {
        &self.chosen_rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewOutput {
    Rows(Box<[ResultOutputRow]>),
    Groups(Box<[ResultGroupSnapshot]>),
}

impl ResultViewOutput {
    pub(crate) fn rows(&self) -> Option<&[ResultOutputRow]> {
        match self {
            Self::Rows(rows) => Some(rows),
            Self::Groups(_) => None,
        }
    }

    pub(crate) fn groups(&self) -> Option<&[ResultGroupSnapshot]> {
        match self {
            Self::Rows(_) => None,
            Self::Groups(groups) => Some(groups),
        }
    }
}

/// Arrival-order-independent commitment to all accepted evaluated inputs at
/// one open or sealed view frontier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResultViewRoot([u8; 32]);

impl ResultViewRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Opaque commitment to an exact result input represented by proof rather
/// than by one [`EvaluatedResultContribution`] per logical row.
///
/// The first producer is the source-image exactness proof. Keeping this root
/// result-generic prevents the pure reducer from depending on that producer's
/// artifact type, while the domain-separated constructor still commits the
/// exact certified population root and cardinality.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CertifiedResultInputRoot([u8; 32]);

impl CertifiedResultInputRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn from_certified_source_population(
        population_root: [u8; 32],
        exact_cardinality: u128,
    ) -> Self {
        let mut hasher = CanonicalHasher::new(CERTIFIED_RESULT_INPUT_ROOT_V1);
        hasher.tag(0x01);
        hasher.digest(population_root);
        hasher.u128(exact_cardinality);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One exact grouped-reducer summary over a proof-certified logical input.
///
/// The source proof owns population coverage. This value carries only the
/// canonical group key and the exact reducer cardinalities needed to project
/// that group without inventing representative input rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CertifiedResultGroupSummary {
    group_values: Box<[ResultValue]>,
    exact_member_count: u128,
    exact_distinct_counts: Box<[u128]>,
}

impl CertifiedResultGroupSummary {
    pub(crate) fn new(
        group_values: Box<[ResultValue]>,
        exact_member_count: u128,
        exact_distinct_counts: Box<[u128]>,
    ) -> Self {
        Self {
            group_values,
            exact_member_count,
            exact_distinct_counts,
        }
    }

    pub(crate) fn group_values(&self) -> &[ResultValue] {
        &self.group_values
    }

    pub(crate) const fn exact_member_count(&self) -> u128 {
        self.exact_member_count
    }

    pub(crate) fn exact_distinct_counts(&self) -> &[u128] {
        &self.exact_distinct_counts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultViewSnapshot {
    spec: ResultViewSpec,
    input_sealed: bool,
    status: ResultViewStatus,
    root: ResultViewRoot,
    counts: ResultViewCounts,
    contributions: Box<[EvaluatedResultContribution]>,
    output: ResultViewOutput,
}

impl ResultViewSnapshot {
    pub(crate) const fn view_id(&self) -> ViewId {
        self.spec.view_id
    }

    pub(crate) const fn spec(&self) -> &ResultViewSpec {
        &self.spec
    }

    pub(crate) const fn input_is_sealed(&self) -> bool {
        self.input_sealed
    }

    pub(crate) const fn status(&self) -> ResultViewStatus {
        self.status
    }

    pub(crate) const fn root(&self) -> ResultViewRoot {
        self.root
    }

    pub(crate) const fn counts(&self) -> ResultViewCounts {
        self.counts
    }

    pub(crate) fn contributions(&self) -> &[EvaluatedResultContribution] {
        &self.contributions
    }

    pub(crate) const fn output(&self) -> &ResultViewOutput {
        &self.output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosedResultView {
    snapshot: ResultViewSnapshot,
}

/// Exact result identity and materialized public output without a second owned
/// copy of every input contribution.
///
/// Durable row evidence remains the authority for the input relation. This
/// compact value is invocation-local preparation for bounded projection
/// records and terminal closure; it never changes snapshot or journal bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompactClosedResultView {
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    root: ResultViewRoot,
    counts: ResultViewCounts,
    output: ResultViewOutput,
}

impl CompactClosedResultView {
    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn root(&self) -> ResultViewRoot {
        self.root
    }

    pub(crate) const fn counts(&self) -> ResultViewCounts {
        self.counts
    }

    pub(crate) const fn output(&self) -> &ResultViewOutput {
        &self.output
    }
}

impl ClosedResultView {
    pub(super) fn restore_from_journal_codec(
        spec: ResultViewSpec,
        contributions: Box<[EvaluatedResultContribution]>,
        output: ResultViewOutput,
    ) -> Result<Self, ResultViewError> {
        spec.validate_spec_root()?;
        let counts = match &output {
            ResultViewOutput::Rows(rows) => ResultViewCounts {
                input_rows: ResultViewCount::Exact(contributions.len() as u128),
                groups: None,
                output_groups: None,
                output_rows: ResultViewCount::Exact(rows.len() as u128),
            },
            ResultViewOutput::Groups(groups) => {
                let included = groups
                    .iter()
                    .filter(|group| group.disposition == ResultGroupDisposition::ExactIncluded)
                    .count() as u128;
                let output_rows = if spec.choice.is_some() {
                    groups
                        .iter()
                        .map(|group| group.chosen_rows.len() as u128)
                        .sum()
                } else {
                    included
                };
                ResultViewCounts {
                    input_rows: ResultViewCount::Exact(contributions.len() as u128),
                    groups: Some(ResultViewCount::Exact(groups.len() as u128)),
                    output_groups: Some(ResultViewCount::Exact(included)),
                    output_rows: ResultViewCount::Exact(output_rows),
                }
            }
        };
        let input_sealed = true;
        let root = result_view_root(spec.spec_root, input_sealed, &contributions, &output);
        let restored = Self {
            snapshot: ResultViewSnapshot {
                spec,
                input_sealed,
                status: ResultViewStatus::Exact,
                root,
                counts,
                contributions,
                output,
            },
        };
        if !restored.validate_identity() {
            return Err(ResultViewError::NonCanonicalRestoredSnapshot);
        }
        Ok(restored)
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.snapshot.spec.view_id
    }

    pub(crate) const fn root(&self) -> ResultViewRoot {
        self.snapshot.root
    }

    pub(crate) const fn counts(&self) -> ResultViewCounts {
        self.snapshot.counts
    }

    pub(crate) const fn snapshot(&self) -> &ResultViewSnapshot {
        &self.snapshot
    }

    /// Revalidate the content identity and closure-only summary fields of a
    /// durable result-view payload. Projection expressions are not rerun;
    /// their complete output is already committed by [`ResultViewRoot`].
    pub(crate) fn validate_identity(&self) -> bool {
        let snapshot = &self.snapshot;
        if snapshot.spec.validate_spec_root().is_err()
            || !snapshot.input_sealed
            || snapshot.status != ResultViewStatus::Exact
            || snapshot
                .contributions
                .windows(2)
                .any(|pair| pair[0].row_id() >= pair[1].row_id())
            || snapshot.root
                != result_view_root(
                    snapshot.spec.spec_root,
                    snapshot.input_sealed,
                    &snapshot.contributions,
                    &snapshot.output,
                )
        {
            return false;
        }

        let input_rows = ResultViewCount::Exact(snapshot.contributions.len() as u128);
        if snapshot.counts.input_rows != input_rows {
            return false;
        }
        match (&snapshot.spec.grain, &snapshot.output) {
            (grain, ResultViewOutput::Rows(rows)) if !grain.is_grouped() => {
                snapshot.counts.groups.is_none()
                    && snapshot.counts.output_groups.is_none()
                    && snapshot.counts.output_rows == ResultViewCount::Exact(rows.len() as u128)
            }
            (grain, ResultViewOutput::Groups(groups)) if grain.is_grouped() => {
                let included = groups
                    .iter()
                    .filter(|group| group.disposition == ResultGroupDisposition::ExactIncluded)
                    .count() as u128;
                let output_rows = if snapshot.spec.choice.is_some() {
                    groups
                        .iter()
                        .map(|group| group.chosen_rows.len() as u128)
                        .sum()
                } else {
                    included
                };
                groups.iter().all(|group| {
                    group.member_count.is_exact()
                        && group
                            .aggregates
                            .iter()
                            .all(|aggregate| aggregate.count.is_exact())
                        && group.disposition.is_exact()
                }) && snapshot.counts.groups == Some(ResultViewCount::Exact(groups.len() as u128))
                    && snapshot.counts.output_groups == Some(ResultViewCount::Exact(included))
                    && snapshot.counts.output_rows == ResultViewCount::Exact(output_rows)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewProjectionError {
    SpecRootMismatch,
    Evaluation {
        message: Box<str>,
    },
    Shape {
        component: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl ResultViewProjectionError {
    pub(crate) fn evaluation(message: impl Into<Box<str>>) -> Self {
        Self::Evaluation {
            message: message.into(),
        }
    }
}

impl fmt::Display for ResultViewProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpecRootMismatch => formatter
                .write_str("result-view spec root does not match its semantic reducer contract"),
            Self::Evaluation { message } => write!(formatter, "{message}"),
            Self::Shape { component, .. } => {
                write!(
                    formatter,
                    "result projector returned the wrong number of {component}"
                )
            }
        }
    }
}

impl Error for ResultViewProjectionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewFinishError {
    InputFrontierOpen,
    Projection(ResultViewProjectionError),
}

impl fmt::Display for ResultViewFinishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputFrontierOpen => {
                formatter.write_str("result view cannot finish while its input frontier is open")
            }
            Self::Projection(error) => error.fmt(formatter),
        }
    }
}

impl Error for ResultViewFinishError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultViewError {
    NonCanonicalRestoredSnapshot,
    SpecRootMismatch,
    InputGrainMismatch,
    EmptyGroupBy,
    AggregateRequiresGroupedGrain,
    HavingRequiresGroupedGrain,
    UnknownHavingMeasure {
        measure_index: usize,
    },
    EmptyParetoObjectives,
    InvalidIntermediateNames,
    InvalidProjectionNames,
    WrongView {
        expected: ViewId,
        actual: ViewId,
    },
    WrongInputKind {
        expected: ResultViewInputKind,
        actual: ResultViewInputKind,
    },
    ContributionShape {
        component: &'static str,
        expected: usize,
        actual: usize,
    },
    InputAlreadySealed,
    ContributionConflict {
        row_id: ResultViewInputRowId,
    },
}

impl fmt::Display for ResultViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalRestoredSnapshot => {
                formatter.write_str("restored result-view snapshot is not canonical")
            }
            Self::SpecRootMismatch => formatter
                .write_str("result-view spec root does not match its semantic reducer contract"),
            Self::InputGrainMismatch => {
                formatter.write_str("result-view grain does not match its input relation")
            }
            Self::EmptyGroupBy => formatter.write_str("result-view group-by key cannot be empty"),
            Self::AggregateRequiresGroupedGrain => {
                formatter.write_str("result-view aggregates require grouped grain")
            }
            Self::HavingRequiresGroupedGrain => {
                formatter.write_str("result-view having requires grouped grain")
            }
            Self::UnknownHavingMeasure { .. } => {
                formatter.write_str("result-view having names an absent measure")
            }
            Self::EmptyParetoObjectives => {
                formatter.write_str("result-view Pareto choice needs at least one objective")
            }
            Self::InvalidIntermediateNames => formatter.write_str(
                "result-view group, measure, and aggregate names must be nonempty and unique",
            ),
            Self::InvalidProjectionNames => {
                formatter.write_str("result-view projection names must be nonempty and unique")
            }
            Self::WrongView { .. } => {
                formatter.write_str("evaluated contribution belongs to another result view")
            }
            Self::WrongInputKind { .. } => {
                formatter.write_str("evaluated contribution has the wrong input-row identity kind")
            }
            Self::ContributionShape { component, .. } => write!(
                formatter,
                "evaluated result-view contribution has the wrong number of {component}"
            ),
            Self::InputAlreadySealed => {
                formatter.write_str("result-view input cannot grow after sealing")
            }
            Self::ContributionConflict { .. } => formatter.write_str(
                "result-view input row was rediscovered with different evaluated values",
            ),
        }
    }
}

impl Error for ResultViewError {}

/// Incremental set reducer for one semantic result view.
#[derive(Clone, Debug)]
pub(crate) struct ResultViewBuilder {
    spec: ResultViewSpec,
    input_sealed: bool,
    contributions: BTreeMap<ResultViewInputRowId, EvaluatedResultContribution>,
}

impl ResultViewBuilder {
    pub(crate) fn new(spec: ResultViewSpec) -> Self {
        Self {
            spec,
            input_sealed: false,
            contributions: BTreeMap::new(),
        }
    }

    pub(crate) const fn spec(&self) -> &ResultViewSpec {
        &self.spec
    }

    pub(crate) const fn input_is_sealed(&self) -> bool {
        self.input_sealed
    }

    pub(crate) fn contribution(
        &self,
        row_id: ResultViewInputRowId,
    ) -> Option<&EvaluatedResultContribution> {
        self.contributions.get(&row_id)
    }

    /// Accept one set member. Equal rediscovery is idempotent, including after
    /// seal; conflicting rediscovery never mutates the reducer.
    pub(crate) fn insert(
        &mut self,
        contribution: EvaluatedResultContribution,
    ) -> Result<bool, ResultViewError> {
        self.validate_contribution(&contribution)?;
        if let Some(existing) = self.contributions.get(&contribution.row_id) {
            return if existing == &contribution {
                Ok(false)
            } else {
                Err(ResultViewError::ContributionConflict {
                    row_id: contribution.row_id,
                })
            };
        }
        if self.input_sealed {
            return Err(ResultViewError::InputAlreadySealed);
        }
        self.contributions.insert(contribution.row_id, contribution);
        Ok(true)
    }

    pub(crate) fn seal_input(&mut self) -> bool {
        let changed = !self.input_sealed;
        self.input_sealed = true;
        changed
    }

    pub(crate) fn snapshot(
        &self,
        projector: &mut impl ResultViewProjector,
    ) -> Result<ResultViewSnapshot, ResultViewProjectionError> {
        self.spec
            .validate_spec_root()
            .map_err(|_| ResultViewProjectionError::SpecRootMismatch)?;
        let contributions = self.contributions.values().cloned().collect::<Vec<_>>();
        let projection =
            project_result_view(&self.spec, self.input_sealed, &contributions, projector)?;
        let root = result_view_root(
            self.spec.spec_root,
            self.input_sealed,
            &contributions,
            &projection.output,
        );
        Ok(ResultViewSnapshot {
            spec: self.spec.clone(),
            input_sealed: self.input_sealed,
            status: if self.input_sealed {
                ResultViewStatus::Exact
            } else {
                ResultViewStatus::Provisional
            },
            root,
            counts: projection.counts,
            contributions: contributions.into_boxed_slice(),
            output: projection.output,
        })
    }

    pub(crate) fn finish(
        &self,
        projector: &mut impl ResultViewProjector,
    ) -> Result<ClosedResultView, ResultViewFinishError> {
        if !self.input_sealed {
            return Err(ResultViewFinishError::InputFrontierOpen);
        }
        Ok(ClosedResultView {
            snapshot: self
                .snapshot(projector)
                .map_err(ResultViewFinishError::Projection)?,
        })
    }

    fn validate_contribution(
        &self,
        contribution: &EvaluatedResultContribution,
    ) -> Result<(), ResultViewError> {
        validate_contribution_for_spec(&self.spec, contribution)
    }
}

fn validate_contribution_for_spec(
    spec: &ResultViewSpec,
    contribution: &EvaluatedResultContribution,
) -> Result<(), ResultViewError> {
    spec.validate_spec_root()?;
    if contribution.view_id != spec.view_id {
        return Err(ResultViewError::WrongView {
            expected: spec.view_id,
            actual: contribution.view_id,
        });
    }
    if contribution.row_id.kind() != spec.input_kind {
        return Err(ResultViewError::WrongInputKind {
            expected: spec.input_kind,
            actual: contribution.row_id.kind(),
        });
    }
    validate_len(
        "group values",
        spec.grain.group_value_count(),
        contribution.group_values.len(),
    )?;
    validate_len(
        "measures",
        spec.measure_names.len(),
        contribution.measures.len(),
    )?;
    validate_len(
        "count-distinct arguments",
        spec.aggregate_names.len(),
        contribution.distinct_arguments.len(),
    )?;
    Ok(())
}

fn validate_borrowed_contributions(
    spec: &ResultViewSpec,
    contributions: &[&EvaluatedResultContribution],
) -> Result<(), ResultViewError> {
    let mut previous = None;
    for contribution in contributions {
        validate_contribution_for_spec(spec, contribution)?;
        if previous.is_some_and(|row_id| contribution.row_id <= row_id) {
            return Err(ResultViewError::NonCanonicalRestoredSnapshot);
        }
        previous = Some(contribution.row_id);
    }
    Ok(())
}

fn validate_len(
    component: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), ResultViewError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ResultViewError::ContributionShape {
            component,
            expected,
            actual,
        })
    }
}

struct ResultProjection {
    counts: ResultViewCounts,
    output: ResultViewOutput,
}

/// Close the exact grouped/no-choice reducer directly over canonical durable
/// contribution references. The caller may drop every freshly reconstructed
/// row state immediately after deterministic equality verification.
pub(crate) fn close_exact_grouped_without_choice_from_borrowed(
    spec: &ResultViewSpec,
    contributions: &[&EvaluatedResultContribution],
    projector: &mut impl ResultViewProjector,
) -> Result<CompactClosedResultView, ResultViewFinishError> {
    if !spec.supports_borrowed_group_close() {
        return Err(ResultViewFinishError::Projection(
            ResultViewProjectionError::evaluation(
                "borrowed result close requires grouped grain without a choice",
            ),
        ));
    }
    spec.validate_spec_root().map_err(|_| {
        ResultViewFinishError::Projection(ResultViewProjectionError::SpecRootMismatch)
    })?;
    validate_borrowed_contributions(spec, contributions).map_err(|error| {
        ResultViewFinishError::Projection(ResultViewProjectionError::evaluation(error.to_string()))
    })?;

    let projection = project_grouped_result_from_borrowed(spec, true, contributions, projector)
        .map_err(ResultViewFinishError::Projection)?;
    let compact = compact_exact_result_view_from_borrowed(spec, contributions, projection.output)
        .map_err(|error| {
        let projection = ResultViewProjectionError::evaluation(error.to_string());
        ResultViewFinishError::Projection(projection)
    })?;
    if compact.counts != projection.counts {
        return Err(ResultViewFinishError::Projection(
            ResultViewProjectionError::evaluation(
                "borrowed result close counts diverged from grouped reduction",
            ),
        ));
    }
    Ok(compact)
}

/// Close the exact one-group image of a proof-certified logical population.
///
/// Unlike [`close_exact_grouped_without_choice_from_borrowed`], this path does
/// not accept a representative singleton contribution. The caller supplies a
/// domain-separated commitment to the complete certified population, its
/// exact logical cardinality, and the one uniform group key. This deliberately
/// narrow seam recognizes one direct `count_distinct(before)`, whose exact
/// value is therefore the population cardinality. Public SELECT evaluation
/// still flows through the ordinary checked [`ResultViewProjector`] exactly
/// once.
pub(crate) fn close_exact_certified_single_group(
    spec: &ResultViewSpec,
    certified_input_root: CertifiedResultInputRoot,
    exact_input_count: u128,
    group_values: &[ResultValue],
    projector: &mut impl ResultViewProjector,
) -> Result<CompactClosedResultView, ResultViewFinishError> {
    let groups = [CertifiedResultGroupSummary::new(
        group_values.to_vec().into_boxed_slice(),
        exact_input_count,
        vec![exact_input_count].into_boxed_slice(),
    )];
    close_exact_certified_groups(
        spec,
        certified_input_root,
        exact_input_count,
        &groups,
        projector,
    )
}

/// Close an exact grouped image of a proof-certified logical population.
///
/// Every public group is supplied as an exact summary. The complete input is
/// still named only by `certified_input_root`; representative rows never enter
/// the reducer or its authenticated identity.
pub(crate) fn close_exact_certified_groups(
    spec: &ResultViewSpec,
    certified_input_root: CertifiedResultInputRoot,
    exact_input_count: u128,
    groups: &[CertifiedResultGroupSummary],
    projector: &mut impl ResultViewProjector,
) -> Result<CompactClosedResultView, ResultViewFinishError> {
    if !spec.grain.is_grouped()
        || spec.choice.is_some()
        || spec.having.is_some()
        || exact_input_count == 0
        || groups.is_empty()
    {
        return Err(ResultViewFinishError::Projection(
            ResultViewProjectionError::evaluation(
                "certified grouped close requires positive grouped input without having or choice",
            ),
        ));
    }
    spec.validate_spec_root().map_err(|_| {
        ResultViewFinishError::Projection(ResultViewProjectionError::SpecRootMismatch)
    })?;

    let mut exact_member_total = 0_u128;
    let mut previous_key: Option<ResultGroupKey> = None;
    let mut output_groups = Vec::with_capacity(groups.len());
    for summary in groups {
        validate_projection_len(
            "certified group values",
            spec.grain.group_value_count(),
            summary.group_values().len(),
        )
        .map_err(ResultViewFinishError::Projection)?;
        validate_projection_len(
            "certified aggregate counts",
            spec.aggregate_names.len(),
            summary.exact_distinct_counts().len(),
        )
        .map_err(ResultViewFinishError::Projection)?;
        if summary.exact_member_count() == 0
            || summary
                .exact_distinct_counts()
                .iter()
                .any(|count| *count == 0 || *count > summary.exact_member_count())
        {
            return Err(ResultViewFinishError::Projection(
                ResultViewProjectionError::evaluation(
                    "certified group counts are empty or exceed group membership",
                ),
            ));
        }
        exact_member_total = exact_member_total
            .checked_add(summary.exact_member_count())
            .ok_or_else(|| {
                ResultViewFinishError::Projection(ResultViewProjectionError::evaluation(
                    "certified group member count overflow",
                ))
            })?;

        let key = ResultGroupKey::new(summary.group_values().to_vec().into_boxed_slice());
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &key)
        {
            return Err(ResultViewFinishError::Projection(
                ResultViewProjectionError::evaluation(
                    "certified group keys are not strictly canonical",
                ),
            ));
        }
        let aggregates = spec
            .aggregate_names
            .iter()
            .zip(summary.exact_distinct_counts())
            .map(|(name, count)| ResultCountDistinctSnapshot {
                name: name.clone(),
                count: ResultViewCount::Exact(*count),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let group = ResultClosedGroupRef {
            key: &key,
            aggregates: &aggregates,
            input_sealed: true,
        };
        let projected_values = projector
            .project_group(group)
            .map_err(ResultViewFinishError::Projection)?;
        validate_projection_len(
            "projected values",
            spec.projection_names.len(),
            projected_values.len(),
        )
        .map_err(ResultViewFinishError::Projection)?;
        previous_key = Some(key.clone());
        output_groups.push(ResultGroupSnapshot {
            key,
            member_count: ResultViewCount::Exact(summary.exact_member_count()),
            observed_having_varies: None,
            disposition: ResultGroupDisposition::ExactIncluded,
            aggregates,
            projected_values: Some(projected_values),
            chosen_rows: Vec::new().into_boxed_slice(),
        });
    }
    if exact_member_total != exact_input_count {
        return Err(ResultViewFinishError::Projection(
            ResultViewProjectionError::evaluation(
                "certified group members do not cover the exact input population",
            ),
        ));
    }

    compact_exact_result_view_from_certified_groups(
        spec,
        certified_input_root,
        exact_input_count,
        groups,
        ResultViewOutput::Groups(output_groups.into_boxed_slice()),
    )
    .map_err(|error| {
        ResultViewFinishError::Projection(ResultViewProjectionError::evaluation(error.to_string()))
    })
}

/// Recompute the compact exact identity of a one-group certified result from
/// its durable public output. This is the later journal/projection bridge: it
/// deliberately receives the certified population commitment rather than a
/// synthetic row identity or a repeated representative contribution. It
/// validates reducer shape and identity only; durable acceptance must still
/// compare the projection records with a fresh checked `close` evaluation.
pub(crate) fn compact_exact_result_view_from_certified_input(
    spec: &ResultViewSpec,
    certified_input_root: CertifiedResultInputRoot,
    exact_input_count: u128,
    expected_group_values: &[ResultValue],
    output: ResultViewOutput,
) -> Result<CompactClosedResultView, ResultViewError> {
    let groups = [CertifiedResultGroupSummary::new(
        expected_group_values.to_vec().into_boxed_slice(),
        exact_input_count,
        vec![exact_input_count].into_boxed_slice(),
    )];
    compact_exact_result_view_from_certified_groups(
        spec,
        certified_input_root,
        exact_input_count,
        &groups,
        output,
    )
}

pub(crate) fn compact_exact_result_view_from_certified_groups(
    spec: &ResultViewSpec,
    certified_input_root: CertifiedResultInputRoot,
    exact_input_count: u128,
    expected_groups: &[CertifiedResultGroupSummary],
    output: ResultViewOutput,
) -> Result<CompactClosedResultView, ResultViewError> {
    spec.validate_spec_root()?;
    if !spec.grain.is_grouped()
        || spec.choice.is_some()
        || spec.having.is_some()
        || exact_input_count == 0
        || expected_groups.is_empty()
    {
        return Err(ResultViewError::NonCanonicalRestoredSnapshot);
    }
    let ResultViewOutput::Groups(groups) = &output else {
        return Err(ResultViewError::NonCanonicalRestoredSnapshot);
    };
    if groups.len() != expected_groups.len() {
        return Err(ResultViewError::NonCanonicalRestoredSnapshot);
    }

    let mut exact_member_total = 0_u128;
    let mut previous_key: Option<&ResultGroupKey> = None;
    for (group, expected) in groups.iter().zip(expected_groups) {
        exact_member_total = exact_member_total
            .checked_add(expected.exact_member_count())
            .ok_or(ResultViewError::NonCanonicalRestoredSnapshot)?;
        if expected.group_values().len() != spec.grain.group_value_count()
            || expected.exact_member_count() == 0
            || expected.exact_distinct_counts().len() != spec.aggregate_names.len()
            || expected
                .exact_distinct_counts()
                .iter()
                .any(|count| *count == 0 || *count > expected.exact_member_count())
            || group.key.values() != expected.group_values()
            || previous_key.is_some_and(|previous| previous >= &group.key)
            || group.member_count != ResultViewCount::Exact(expected.exact_member_count())
            || group.observed_having_varies.is_some()
            || group.disposition != ResultGroupDisposition::ExactIncluded
            || group.aggregates.len() != spec.aggregate_names.len()
            || group
                .aggregates
                .iter()
                .zip(spec.aggregate_names.iter())
                .zip(expected.exact_distinct_counts())
                .any(|((aggregate, name), count)| {
                    aggregate.name.as_ref() != name.as_ref()
                        || aggregate.count != ResultViewCount::Exact(*count)
                })
            || group.projected_values.as_ref().map(|values| values.len())
                != Some(spec.projection_names.len())
            || !group.chosen_rows.is_empty()
        {
            return Err(ResultViewError::NonCanonicalRestoredSnapshot);
        }
        previous_key = Some(&group.key);
    }
    if exact_member_total != exact_input_count {
        return Err(ResultViewError::NonCanonicalRestoredSnapshot);
    }

    let counts = ResultViewCounts {
        input_rows: ResultViewCount::Exact(exact_input_count),
        groups: Some(ResultViewCount::Exact(expected_groups.len() as u128)),
        output_groups: Some(ResultViewCount::Exact(expected_groups.len() as u128)),
        output_rows: ResultViewCount::Exact(expected_groups.len() as u128),
    };
    let root = certified_result_view_root(
        spec.spec_root,
        certified_input_root,
        exact_input_count,
        &output,
    );
    Ok(CompactClosedResultView {
        view_id: spec.view_id,
        spec_root: spec.spec_root,
        root,
        counts,
        output,
    })
}

/// Recompute an exact result root and closure counts over borrowed canonical
/// inputs plus materialized output. This is the durable-publication validator;
/// it deliberately does not construct a [`ClosedResultView`].
pub(crate) fn compact_exact_result_view_from_borrowed(
    spec: &ResultViewSpec,
    contributions: &[&EvaluatedResultContribution],
    output: ResultViewOutput,
) -> Result<CompactClosedResultView, ResultViewError> {
    spec.validate_spec_root()?;
    validate_borrowed_contributions(spec, contributions)?;
    let counts = exact_result_view_counts(spec, contributions.len(), &output)?;
    let root = result_view_root_from_borrowed(
        spec.spec_root,
        true,
        contributions.iter().copied(),
        contributions.len(),
        &output,
    );
    Ok(CompactClosedResultView {
        view_id: spec.view_id,
        spec_root: spec.spec_root,
        root,
        counts,
        output,
    })
}

fn project_result_view(
    spec: &ResultViewSpec,
    input_sealed: bool,
    contributions: &[EvaluatedResultContribution],
    projector: &mut impl ResultViewProjector,
) -> Result<ResultProjection, ResultViewProjectionError> {
    if spec.grain.is_grouped() {
        project_grouped_result(spec, input_sealed, contributions, projector)
    } else {
        project_row_result(spec, input_sealed, contributions, projector)
    }
}

fn project_row_result(
    spec: &ResultViewSpec,
    input_sealed: bool,
    contributions: &[EvaluatedResultContribution],
    projector: &mut impl ResultViewProjector,
) -> Result<ResultProjection, ResultViewProjectionError> {
    let output_rows = if let Some(choice) = spec.choice.as_ref() {
        let mut candidates = Vec::with_capacity(contributions.len());
        for contribution in contributions {
            candidates.push(evaluate_choice_candidate(
                spec,
                contribution,
                None,
                projector,
            )?);
        }
        let mut output = Vec::new();
        for candidate in choose_rows(choice, &candidates) {
            output.push(project_output_row(
                spec,
                candidate.contribution,
                candidate.group,
                projector,
            )?);
        }
        output
    } else {
        let mut output = Vec::with_capacity(contributions.len());
        for contribution in contributions {
            output.push(project_output_row(spec, contribution, None, projector)?);
        }
        output
    };
    let input_count = contributions.len() as u128;
    let output_count = output_rows.len() as u128;
    Ok(ResultProjection {
        counts: ResultViewCounts {
            input_rows: exact_or_lower(input_sealed, input_count),
            groups: None,
            output_groups: None,
            output_rows: if input_sealed {
                ResultViewCount::Exact(output_count)
            } else if spec.choice.is_some() {
                ResultViewCount::Provisional(output_count)
            } else {
                ResultViewCount::LowerBound(output_count)
            },
        },
        output: ResultViewOutput::Rows(output_rows.into_boxed_slice()),
    })
}

fn project_grouped_result(
    spec: &ResultViewSpec,
    input_sealed: bool,
    contributions: &[EvaluatedResultContribution],
    projector: &mut impl ResultViewProjector,
) -> Result<ResultProjection, ResultViewProjectionError> {
    let borrowed = contributions.iter().collect::<Vec<_>>();
    project_grouped_result_from_borrowed(spec, input_sealed, &borrowed, projector)
}

fn project_grouped_result_from_borrowed(
    spec: &ResultViewSpec,
    input_sealed: bool,
    contributions: &[&EvaluatedResultContribution],
    projector: &mut impl ResultViewProjector,
) -> Result<ResultProjection, ResultViewProjectionError> {
    let mut grouped = BTreeMap::<ResultGroupKey, Vec<&EvaluatedResultContribution>>::new();
    if matches!(&spec.grain, ResultViewGrain::GroupAll) {
        grouped.insert(
            ResultGroupKey::new(Vec::new().into_boxed_slice()),
            Vec::new(),
        );
    }
    for contribution in contributions.iter().copied() {
        grouped
            .entry(ResultGroupKey::new(contribution.group_values.clone()))
            .or_default()
            .push(contribution);
    }

    let mut output_group_count = 0_u128;
    let mut output_row_count = 0_u128;
    let mut groups = Vec::with_capacity(grouped.len());
    for (key, members) in grouped {
        let observed_having_varies = spec.having.map(|having| match having {
            ResultViewHaving::Varies { measure_index } => measure_varies(&members, measure_index),
        });
        let currently_passes = observed_having_varies.unwrap_or(true);
        let disposition = if input_sealed {
            if currently_passes {
                ResultGroupDisposition::ExactIncluded
            } else {
                ResultGroupDisposition::ExactExcluded
            }
        } else {
            ResultGroupDisposition::Provisional {
                currently_passes_having: currently_passes,
            }
        };

        let aggregates = spec
            .aggregate_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let distinct = members
                    .iter()
                    .map(|member| &member.distinct_arguments[index])
                    .collect::<BTreeSet<_>>()
                    .len() as u128;
                ResultCountDistinctSnapshot {
                    name: name.clone(),
                    count: exact_or_lower(input_sealed, distinct),
                }
            })
            .collect::<Vec<_>>();

        let group_ref = ResultClosedGroupRef {
            key: &key,
            aggregates: &aggregates,
            input_sealed,
        };
        let (projected_values, chosen_rows) = if !currently_passes {
            (None, Vec::new())
        } else if let Some(choice) = spec.choice.as_ref() {
            let mut candidates = Vec::with_capacity(members.len());
            for member in &members {
                candidates.push(evaluate_choice_candidate(
                    spec,
                    member,
                    Some(group_ref),
                    projector,
                )?);
            }
            let mut chosen = Vec::new();
            for candidate in choose_rows(choice, &candidates) {
                chosen.push(project_output_row(
                    spec,
                    candidate.contribution,
                    candidate.group,
                    projector,
                )?);
            }
            (None, chosen)
        } else {
            let values = projector.project_group(group_ref)?;
            validate_projection_len(
                "projected values",
                spec.projection_names.len(),
                values.len(),
            )?;
            (Some(values), Vec::new())
        };
        if currently_passes {
            output_group_count += 1;
            output_row_count += if spec.choice.is_some() {
                chosen_rows.len() as u128
            } else {
                1
            };
        }

        groups.push(ResultGroupSnapshot {
            key,
            member_count: exact_or_lower(input_sealed, members.len() as u128),
            observed_having_varies,
            disposition,
            aggregates: aggregates.into_boxed_slice(),
            projected_values,
            chosen_rows: chosen_rows.into_boxed_slice(),
        });
    }

    let group_count = groups.len() as u128;
    let output_is_provisional = !input_sealed && (spec.having.is_some() || spec.choice.is_some());
    Ok(ResultProjection {
        counts: ResultViewCounts {
            input_rows: exact_or_lower(input_sealed, contributions.len() as u128),
            groups: Some(exact_or_lower(input_sealed, group_count)),
            output_groups: Some(if input_sealed {
                ResultViewCount::Exact(output_group_count)
            } else if spec.having.is_some() {
                ResultViewCount::Provisional(output_group_count)
            } else {
                ResultViewCount::LowerBound(output_group_count)
            }),
            output_rows: if input_sealed {
                ResultViewCount::Exact(output_row_count)
            } else if output_is_provisional {
                ResultViewCount::Provisional(output_row_count)
            } else {
                ResultViewCount::LowerBound(output_row_count)
            },
        },
        output: ResultViewOutput::Groups(groups.into_boxed_slice()),
    })
}

fn exact_or_lower(exact: bool, value: u128) -> ResultViewCount {
    if exact {
        ResultViewCount::Exact(value)
    } else {
        ResultViewCount::LowerBound(value)
    }
}

fn exact_result_view_counts(
    spec: &ResultViewSpec,
    input_count: usize,
    output: &ResultViewOutput,
) -> Result<ResultViewCounts, ResultViewError> {
    match (&spec.grain, output) {
        (grain, ResultViewOutput::Rows(rows)) if !grain.is_grouped() => Ok(ResultViewCounts {
            input_rows: ResultViewCount::Exact(input_count as u128),
            groups: None,
            output_groups: None,
            output_rows: ResultViewCount::Exact(rows.len() as u128),
        }),
        (grain, ResultViewOutput::Groups(groups)) if grain.is_grouped() => {
            if groups.iter().any(|group| {
                !group.member_count.is_exact()
                    || !group.disposition.is_exact()
                    || group
                        .aggregates
                        .iter()
                        .any(|aggregate| !aggregate.count.is_exact())
            }) {
                return Err(ResultViewError::NonCanonicalRestoredSnapshot);
            }
            let included = groups
                .iter()
                .filter(|group| group.disposition == ResultGroupDisposition::ExactIncluded)
                .count() as u128;
            let output_rows = if spec.choice.is_some() {
                groups
                    .iter()
                    .map(|group| group.chosen_rows.len() as u128)
                    .sum()
            } else {
                included
            };
            Ok(ResultViewCounts {
                input_rows: ResultViewCount::Exact(input_count as u128),
                groups: Some(ResultViewCount::Exact(groups.len() as u128)),
                output_groups: Some(ResultViewCount::Exact(included)),
                output_rows: ResultViewCount::Exact(output_rows),
            })
        }
        _ => Err(ResultViewError::NonCanonicalRestoredSnapshot),
    }
}

fn measure_varies(members: &[&EvaluatedResultContribution], measure_index: usize) -> bool {
    let Some(first) = members.first() else {
        return false;
    };
    members
        .iter()
        .skip(1)
        .any(|member| member.measures[measure_index] != first.measures[measure_index])
}

struct ChoiceCandidate<'a> {
    contribution: &'a EvaluatedResultContribution,
    group: Option<ResultClosedGroupRef<'a>>,
    objectives: Box<[i64]>,
}

fn evaluate_choice_candidate<'a>(
    spec: &ResultViewSpec,
    contribution: &'a EvaluatedResultContribution,
    group: Option<ResultClosedGroupRef<'a>>,
    projector: &mut impl ResultViewProjector,
) -> Result<ChoiceCandidate<'a>, ResultViewProjectionError> {
    let objectives = projector.evaluate_objectives(ResultClosedRowRef {
        contribution,
        group,
    })?;
    validate_projection_len(
        "choice objectives",
        spec.objective_count(),
        objectives.len(),
    )?;
    Ok(ChoiceCandidate {
        contribution,
        group,
        objectives,
    })
}

fn project_output_row(
    spec: &ResultViewSpec,
    contribution: &EvaluatedResultContribution,
    group: Option<ResultClosedGroupRef<'_>>,
    projector: &mut impl ResultViewProjector,
) -> Result<ResultOutputRow, ResultViewProjectionError> {
    let values = projector.project_row(ResultClosedRowRef {
        contribution,
        group,
    })?;
    validate_projection_len(
        "projected values",
        spec.projection_names.len(),
        values.len(),
    )?;
    Ok(ResultOutputRow {
        row_id: contribution.row_id,
        values,
    })
}

fn validate_projection_len(
    component: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), ResultViewProjectionError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ResultViewProjectionError::Shape {
            component,
            expected,
            actual,
        })
    }
}

fn choose_rows<'rows, 'input>(
    choice: &ResultViewChoice,
    rows: &'rows [ChoiceCandidate<'input>],
) -> Vec<&'rows ChoiceCandidate<'input>> {
    match choice {
        ResultViewChoice::Optimize {
            cardinality,
            direction,
        } => {
            let best = rows
                .iter()
                .map(|row| row.objectives[0])
                .reduce(|best, candidate| match direction {
                    ExploreOptimizeDirection::Minimize => best.min(candidate),
                    ExploreOptimizeDirection::Maximize => best.max(candidate),
                });
            let Some(best) = best else {
                return Vec::new();
            };
            let tied = rows.iter().filter(|row| row.objectives[0] == best);
            match cardinality {
                ExploreChooseCardinality::One => tied
                    .min_by_key(|row| row.contribution.row_id)
                    .into_iter()
                    .collect(),
                ExploreChooseCardinality::All => tied.collect(),
            }
        }
        ResultViewChoice::Pareto { directions } => {
            let mut frontier = Vec::<&ChoiceCandidate<'_>>::new();
            for candidate in rows {
                if frontier
                    .iter()
                    .any(|existing| dominates(existing, candidate, directions))
                {
                    continue;
                }
                frontier.retain(|existing| !dominates(candidate, existing, directions));
                frontier.push(candidate);
            }
            frontier.sort_by_key(|row| row.contribution.row_id);
            frontier
        }
    }
}

fn dominates(
    left: &ChoiceCandidate<'_>,
    right: &ChoiceCandidate<'_>,
    directions: &[ExploreOptimizeDirection],
) -> bool {
    let mut strictly_better = false;
    for ((left, right), direction) in left
        .objectives
        .iter()
        .zip(right.objectives.iter())
        .zip(directions.iter())
    {
        match direction {
            ExploreOptimizeDirection::Minimize => {
                if left > right {
                    return false;
                }
                strictly_better |= left < right;
            }
            ExploreOptimizeDirection::Maximize => {
                if left < right {
                    return false;
                }
                strictly_better |= left > right;
            }
        }
    }
    strictly_better
}

fn derive_result_view_spec_root(spec: &ResultViewSpec) -> ResultViewSpecRoot {
    let mut hasher = CanonicalHasher::new(RESULT_VIEW_SPEC_ROOT_HASH_V1);

    hasher.tag(0x01);
    hasher.digest(spec.view_id.bytes());

    hasher.tag(0x02);
    hasher.tag(match spec.input_kind {
        ResultViewInputKind::Source => 0x03,
        ResultViewInputKind::Case => 0x01,
        ResultViewInputKind::Incidence => 0x02,
    });

    hasher.tag(0x03);
    match &spec.grain {
        ResultViewGrain::EachCase => hasher.tag(0x01),
        ResultViewGrain::EachIncidence => hasher.tag(0x02),
        ResultViewGrain::GroupAll => hasher.tag(0x03),
        ResultViewGrain::GroupBy { field_names } => {
            hasher.tag(0x04);
            hash_names(&mut hasher, field_names);
        }
    }

    hasher.tag(0x04);
    hash_names(&mut hasher, &spec.measure_names);
    hasher.tag(0x05);
    hash_names(&mut hasher, &spec.aggregate_names);
    hasher.tag(0x06);
    hash_names(&mut hasher, &spec.projection_names);

    hasher.tag(0x07);
    match spec.having {
        None => hasher.tag(0x00),
        Some(ResultViewHaving::Varies { measure_index }) => {
            hasher.tag(0x01);
            hasher.u128(measure_index as u128);
        }
    }

    hasher.tag(0x08);
    match &spec.choice {
        None => hasher.tag(0x00),
        Some(ResultViewChoice::Optimize {
            cardinality,
            direction,
        }) => {
            hasher.tag(0x01);
            hasher.tag(match cardinality {
                ExploreChooseCardinality::One => 0x01,
                ExploreChooseCardinality::All => 0x02,
            });
            hash_direction(&mut hasher, *direction);
        }
        Some(ResultViewChoice::Pareto { directions }) => {
            hasher.tag(0x02);
            hasher.u128(directions.len() as u128);
            for direction in directions.iter().copied() {
                hash_direction(&mut hasher, direction);
            }
        }
    }

    ResultViewSpecRoot(hasher.finish())
}

fn hash_names(hasher: &mut CanonicalHasher, names: &[Box<str>]) {
    hasher.u128(names.len() as u128);
    for name in names {
        hasher.bytes(name.as_bytes());
    }
}

fn hash_direction(hasher: &mut CanonicalHasher, direction: ExploreOptimizeDirection) {
    hasher.tag(match direction {
        ExploreOptimizeDirection::Minimize => 0x01,
        ExploreOptimizeDirection::Maximize => 0x02,
    });
}

fn result_view_root(
    spec_root: ResultViewSpecRoot,
    input_sealed: bool,
    contributions: &[EvaluatedResultContribution],
    output: &ResultViewOutput,
) -> ResultViewRoot {
    result_view_root_from_borrowed(
        spec_root,
        input_sealed,
        contributions.iter(),
        contributions.len(),
        output,
    )
}

fn certified_result_view_root(
    spec_root: ResultViewSpecRoot,
    certified_input_root: CertifiedResultInputRoot,
    exact_input_count: u128,
    output: &ResultViewOutput,
) -> ResultViewRoot {
    let mut hasher = CanonicalHasher::new(CERTIFIED_RESULT_VIEW_ROOT_V1);
    hasher.digest(spec_root.bytes());
    hasher.digest(certified_input_root.bytes());
    hasher.u128(exact_input_count);
    hash_output(&mut hasher, output);
    ResultViewRoot(hasher.finish())
}

fn result_view_root_from_borrowed<'a>(
    spec_root: ResultViewSpecRoot,
    input_sealed: bool,
    contributions: impl IntoIterator<Item = &'a EvaluatedResultContribution>,
    contribution_count: usize,
    output: &ResultViewOutput,
) -> ResultViewRoot {
    let mut hasher = CanonicalHasher::new(RESULT_VIEW_ROOT_HASH_V3);
    hasher.digest(spec_root.bytes());
    hasher.tag(u8::from(input_sealed));
    hasher.u128(contribution_count as u128);
    for contribution in contributions {
        hash_row_id(&mut hasher, contribution.row_id);
        hash_values(&mut hasher, &contribution.group_values);
        hash_values(&mut hasher, &contribution.measures);
        hash_values(&mut hasher, &contribution.distinct_arguments);
    }
    hash_output(&mut hasher, output);
    ResultViewRoot(hasher.finish())
}

fn hash_output(hasher: &mut CanonicalHasher, output: &ResultViewOutput) {
    match output {
        ResultViewOutput::Rows(rows) => {
            hasher.tag(0x01);
            hasher.u128(rows.len() as u128);
            for row in rows {
                hash_output_row(hasher, row);
            }
        }
        ResultViewOutput::Groups(groups) => {
            hasher.tag(0x02);
            hasher.u128(groups.len() as u128);
            for group in groups {
                hash_values(hasher, group.key.values());
                hash_count(hasher, group.member_count);
                match group.observed_having_varies {
                    None => hasher.tag(0x00),
                    Some(value) => {
                        hasher.tag(0x01);
                        hasher.tag(u8::from(value));
                    }
                }
                match group.disposition {
                    ResultGroupDisposition::Provisional {
                        currently_passes_having,
                    } => {
                        hasher.tag(0x01);
                        hasher.tag(u8::from(currently_passes_having));
                    }
                    ResultGroupDisposition::ExactIncluded => hasher.tag(0x02),
                    ResultGroupDisposition::ExactExcluded => hasher.tag(0x03),
                }
                hasher.u128(group.aggregates.len() as u128);
                for aggregate in &group.aggregates {
                    hasher.bytes(aggregate.name.as_bytes());
                    hash_count(hasher, aggregate.count);
                }
                match &group.projected_values {
                    None => hasher.tag(0x00),
                    Some(values) => {
                        hasher.tag(0x01);
                        hash_values(hasher, values);
                    }
                }
                hasher.u128(group.chosen_rows.len() as u128);
                for row in &group.chosen_rows {
                    hash_output_row(hasher, row);
                }
            }
        }
    }
}

fn hash_output_row(hasher: &mut CanonicalHasher, row: &ResultOutputRow) {
    hash_row_id(hasher, row.row_id);
    hash_values(hasher, &row.values);
}

fn hash_count(hasher: &mut CanonicalHasher, count: ResultViewCount) {
    match count {
        ResultViewCount::LowerBound(value) => {
            hasher.tag(0x01);
            hasher.u128(value);
        }
        ResultViewCount::Provisional(value) => {
            hasher.tag(0x02);
            hasher.u128(value);
        }
        ResultViewCount::Exact(value) => {
            hasher.tag(0x03);
            hasher.u128(value);
        }
    }
}

fn hash_row_id(hasher: &mut CanonicalHasher, row_id: ResultViewInputRowId) {
    match row_id {
        ResultViewInputRowId::Source(source_key) => {
            hasher.tag(0x03);
            hasher.digest(source_key.bytes());
        }
        ResultViewInputRowId::Case(case_id) => {
            hasher.tag(0x01);
            hasher.digest(case_id.bytes());
        }
        ResultViewInputRowId::Incidence(incidence) => {
            hasher.tag(0x02);
            hasher.digest(incidence.case_id.bytes());
            hasher.digest(incidence.transition_id.bytes());
            hasher.digest(incidence.signature_id.request_id().bytes());
            hasher.digest(incidence.signature_id.bytes());
        }
    }
}

fn hash_values(hasher: &mut CanonicalHasher, values: &[ResultValue]) {
    hasher.u128(values.len() as u128);
    for value in values {
        match value {
            ResultValue::Value(value) => {
                hasher.tag(0x01);
                hasher.digest(canonical_explore_value_digest(value));
            }
            ResultValue::CaseId(case_id) => {
                hasher.tag(0x02);
                hasher.digest(case_id.bytes());
            }
            ResultValue::TransitionId(transition_id) => {
                hasher.tag(0x03);
                hasher.digest(transition_id.bytes());
            }
            ResultValue::SignatureId(signature_id) => {
                hasher.tag(0x04);
                hasher.digest(signature_id.request_id().bytes());
                hasher.digest(signature_id.bytes());
            }
            ResultValue::StructuralMechanismId(mechanism_id) => {
                hasher.tag(0x05);
                hasher.digest(mechanism_id.bytes());
            }
            ResultValue::ExecutionProfileId(profile_id) => {
                hasher.tag(0x06);
                hasher.digest(profile_id.bytes());
            }
        }
    }
}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
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
    use crate::explore::mechanism_incidence::MechanismSignatureDefinition;
    use crate::explore::relation::{
        AdmissionId, FindPolarity, MechanismRequestId, MechanismTargetId, QuestionId, RelationId,
        RelationLineageId, RelationProvenance, RelationSupportId, SourceKey, SourceRow,
        SuccessorKey, SuccessorRow, ViewInputId,
    };

    fn names(values: &[&str]) -> Box<[Box<str>]> {
        values
            .iter()
            .map(|value| Box::<str>::from(*value))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    fn identities(name: &str) -> (RelationId, QuestionId, MechanismRequestId) {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(format!("relation-{name}").as_bytes());
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"selected-cases",
            FindPolarity::All,
        );
        let request_id = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Selected,
            b"assess-policy",
            b"dynamic-control-v1",
        );
        (relation_id, question_id, request_id)
    }

    #[test]
    fn spec_root_is_sensitive_to_every_semantic_reducer_field() {
        let (_, question_id, _) = identities("spec-root");
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"semantic-result-view",
        );
        let base = ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Case,
            ResultViewGrain::GroupBy {
                field_names: names(&["bin"]),
            },
            names(&["loss", "tax"]),
            names(&["cases"]),
            names(&["municipality"]),
            Some(ResultViewHaving::Varies { measure_index: 0 }),
            Some(ResultViewChoice::Optimize {
                cardinality: ExploreChooseCardinality::One,
                direction: ExploreOptimizeDirection::Minimize,
            }),
        )
        .unwrap();
        assert_eq!(base.validate_spec_root(), Ok(()));

        let mut variants = Vec::new();
        let mut variant = base.clone();
        variant.view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"other-result-view",
        );
        variants.push(variant);
        let mut variant = base.clone();
        variant.input_kind = ResultViewInputKind::Incidence;
        variants.push(variant);
        let mut variant = base.clone();
        variant.grain = ResultViewGrain::GroupAll;
        variants.push(variant);
        let mut variant = base.clone();
        variant.grain = ResultViewGrain::GroupBy {
            field_names: names(&["other_bin"]),
        };
        variants.push(variant);
        let mut variant = base.clone();
        variant.measure_names = names(&["other_loss", "tax"]);
        variants.push(variant);
        let mut variant = base.clone();
        variant.aggregate_names = names(&["people"]);
        variants.push(variant);
        let mut variant = base.clone();
        variant.projection_names = names(&["commune"]);
        variants.push(variant);
        let mut variant = base.clone();
        variant.having = Some(ResultViewHaving::Varies { measure_index: 1 });
        variants.push(variant);
        let mut variant = base.clone();
        variant.choice = Some(ResultViewChoice::Optimize {
            cardinality: ExploreChooseCardinality::All,
            direction: ExploreOptimizeDirection::Minimize,
        });
        variants.push(variant);
        let mut variant = base.clone();
        variant.choice = Some(ResultViewChoice::Optimize {
            cardinality: ExploreChooseCardinality::One,
            direction: ExploreOptimizeDirection::Maximize,
        });
        variants.push(variant);
        let mut variant = base.clone();
        variant.choice = Some(ResultViewChoice::Pareto {
            directions: vec![
                ExploreOptimizeDirection::Minimize,
                ExploreOptimizeDirection::Maximize,
            ]
            .into_boxed_slice(),
        });
        variants.push(variant);

        for variant in variants {
            assert_ne!(base.spec_root(), derive_result_view_spec_root(&variant));
        }

        let mut pareto_left = base.clone();
        pareto_left.choice = Some(ResultViewChoice::Pareto {
            directions: vec![
                ExploreOptimizeDirection::Minimize,
                ExploreOptimizeDirection::Maximize,
            ]
            .into_boxed_slice(),
        });
        let mut pareto_right = pareto_left.clone();
        pareto_right.choice = Some(ResultViewChoice::Pareto {
            directions: vec![
                ExploreOptimizeDirection::Maximize,
                ExploreOptimizeDirection::Minimize,
            ]
            .into_boxed_slice(),
        });
        assert_ne!(
            derive_result_view_spec_root(&pareto_left),
            derive_result_view_spec_root(&pareto_right)
        );

        let mut tampered = base.clone();
        tampered.measure_names = names(&["tampered", "tax"]);
        assert_eq!(
            tampered.validate_spec_root(),
            Err(ResultViewError::SpecRootMismatch)
        );
    }

    fn provenance(name: &str) -> RelationProvenance {
        RelationProvenance::new(
            [RelationLineageId::from_canonical_preimage(
                format!("lineage-{name}").as_bytes(),
            )],
            [RelationSupportId::from_canonical_preimage(
                format!("support-{name}").as_bytes(),
            )],
        )
    }

    fn case(relation_id: RelationId, name: &str, before: i64, after: i64) -> RelationalCaseId {
        let source = SourceRow::new(
            ExploreValue::String("scenario".to_string()),
            ExploreValue::Tuple(vec![
                ExploreValue::String(name.to_string()),
                ExploreValue::Int(before),
            ]),
            provenance(&format!("source-{name}")),
        );
        let source_key = SourceKey::derive(relation_id, &source);
        let successor = SuccessorRow::new(
            ExploreValue::Tuple(vec![
                ExploreValue::String(name.to_string()),
                ExploreValue::Int(after),
            ]),
            provenance(&format!("successor-{name}")),
        );
        let successor_key = SuccessorKey::derive(relation_id, source_key, &successor);
        RelationalCaseId::derive(relation_id, source_key, successor_key)
    }

    fn transition(name: &str) -> TransitionId {
        TransitionId::from_bytes(Sha256::digest(format!("transition-{name}")).into())
    }

    fn value(value: ExploreValue) -> ResultValue {
        ResultValue::Value(value)
    }

    #[derive(Default)]
    struct FixtureProjector {
        rows: BTreeMap<ResultViewInputRowId, FixtureProjection>,
        group_values: Box<[ResultValue]>,
    }

    #[derive(Clone)]
    struct FixtureProjection {
        values: Box<[ResultValue]>,
        objectives: Box<[i64]>,
    }

    impl ResultViewProjector for FixtureProjector {
        fn project_group(
            &mut self,
            _group: ResultClosedGroupRef<'_>,
        ) -> Result<Box<[ResultValue]>, ResultViewProjectionError> {
            Ok(self.group_values.clone())
        }

        fn evaluate_objectives(
            &mut self,
            row: ResultClosedRowRef<'_>,
        ) -> Result<Box<[i64]>, ResultViewProjectionError> {
            self.rows
                .get(&row.contribution().row_id())
                .map(|projection| projection.objectives.clone())
                .ok_or_else(|| ResultViewProjectionError::evaluation("missing fixture row"))
        }

        fn project_row(
            &mut self,
            row: ResultClosedRowRef<'_>,
        ) -> Result<Box<[ResultValue]>, ResultViewProjectionError> {
            self.rows
                .get(&row.contribution().row_id())
                .map(|projection| projection.values.clone())
                .ok_or_else(|| ResultViewProjectionError::evaluation("missing fixture row"))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn contribution(
        projector: &mut FixtureProjector,
        view_id: ViewId,
        row_id: ResultViewInputRowId,
        group_values: Vec<ResultValue>,
        measures: Vec<ResultValue>,
        distinct_arguments: Vec<ResultValue>,
        row_values: Vec<ResultValue>,
        objectives: Vec<i64>,
    ) -> EvaluatedResultContribution {
        projector.rows.insert(
            row_id,
            FixtureProjection {
                values: row_values.into_boxed_slice(),
                objectives: objectives.into_boxed_slice(),
            },
        );
        EvaluatedResultContribution::new(
            view_id,
            row_id,
            group_values,
            measures,
            distinct_arguments,
        )
    }

    #[test]
    fn shared_signature_counts_once_in_an_ordinary_fifty_dkk_group() {
        let (relation_id, _, request_id) = identities("loss-group");
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::MechanismIncidence(request_id),
            b"loss-group-view",
        );
        let spec = ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Incidence,
            ResultViewGrain::GroupBy {
                field_names: names(&["bin_start_ore"]),
            },
            names(&[]),
            names(&["mechanisms", "cases"]),
            names(&[]),
            None,
            None,
        )
        .unwrap();

        let carl = case(relation_id, "Carl", 199_999, 200_000);
        let john = case(relation_id, "John", 9_999, 10_000);
        let definition = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"shared-complete-differential-signature".as_slice(),
        );
        let carl_row = ResultViewInputRowId::Incidence(MechanismIncidenceRowId::new(
            carl,
            transition("Carl"),
            definition.id(),
        ));
        let john_row = ResultViewInputRowId::Incidence(MechanismIncidenceRowId::new(
            john,
            transition("John"),
            definition.id(),
        ));
        let mut projector = FixtureProjector::default();
        let carl_contribution = contribution(
            &mut projector,
            view_id,
            carl_row,
            vec![value(ExploreValue::Int(5_000))],
            vec![],
            vec![definition.id().into(), carl.into()],
            vec![],
            vec![],
        );
        let john_contribution = contribution(
            &mut projector,
            view_id,
            john_row,
            vec![value(ExploreValue::Int(5_000))],
            vec![],
            vec![definition.id().into(), john.into()],
            vec![],
            vec![],
        );

        let mut builder = ResultViewBuilder::new(spec);
        assert!(builder.insert(carl_contribution.clone()).unwrap());
        assert!(!builder.insert(carl_contribution).unwrap());
        assert!(builder.insert(john_contribution).unwrap());
        let open = builder.snapshot(&mut projector).unwrap();
        assert_eq!(open.status(), ResultViewStatus::Provisional);
        assert_eq!(open.counts().input_rows(), ResultViewCount::LowerBound(2));
        assert_eq!(open.counts().groups(), Some(ResultViewCount::LowerBound(1)));
        let group = &open.output().groups().unwrap()[0];
        assert_eq!(group.aggregates[0].count(), ResultViewCount::LowerBound(1));
        assert_eq!(group.aggregates[1].count(), ResultViewCount::LowerBound(2));

        builder.seal_input();
        let closed = builder.finish(&mut projector).unwrap();
        let group = &closed.snapshot().output().groups().unwrap()[0];
        assert_eq!(group.member_count(), ResultViewCount::Exact(2));
        assert_eq!(group.disposition(), ResultGroupDisposition::ExactIncluded);
        assert_eq!(group.aggregates[0].name(), "mechanisms");
        assert_eq!(group.aggregates[0].count(), ResultViewCount::Exact(1));
        assert_eq!(group.aggregates[1].count(), ResultViewCount::Exact(2));
        assert!(group.chosen_rows().is_empty());
        assert_ne!(carl_row, john_row);
    }

    #[test]
    fn municipality_choose_all_preserves_every_tied_minimum() {
        let (relation_id, question_id, _) = identities("municipality-ties");
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"municipality-minimum-view",
        );
        let spec = ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Case,
            ResultViewGrain::GroupAll,
            names(&["tax_ore"]),
            names(&[]),
            names(&["municipality", "tax_ore"]),
            Some(ResultViewHaving::Varies { measure_index: 0 }),
            Some(ResultViewChoice::Optimize {
                cardinality: ExploreChooseCardinality::All,
                direction: ExploreOptimizeDirection::Minimize,
            }),
        )
        .unwrap();
        let copenhagen = case(relation_id, "Copenhagen", 0, 1);
        let aarhus = case(relation_id, "Aarhus", 0, 2);
        let odense = case(relation_id, "Odense", 0, 3);
        let mut projector = FixtureProjector::default();
        let mut builder = ResultViewBuilder::new(spec);
        for (case_id, municipality, tax) in [
            (copenhagen, "Copenhagen", 100_000),
            (aarhus, "Aarhus", 100_000),
            (odense, "Odense", 120_000),
        ] {
            builder
                .insert(contribution(
                    &mut projector,
                    view_id,
                    ResultViewInputRowId::Case(case_id),
                    vec![],
                    vec![value(ExploreValue::Int(tax))],
                    vec![],
                    vec![
                        value(ExploreValue::String(municipality.to_string())),
                        value(ExploreValue::Int(tax)),
                    ],
                    vec![tax],
                ))
                .unwrap();
        }
        assert_eq!(
            builder
                .snapshot(&mut projector)
                .unwrap()
                .counts()
                .output_rows(),
            ResultViewCount::Provisional(2)
        );
        builder.seal_input();
        let closed = builder.finish(&mut projector).unwrap();
        let group = &closed.snapshot().output().groups().unwrap()[0];
        assert_eq!(group.disposition(), ResultGroupDisposition::ExactIncluded);
        assert_eq!(group.chosen_rows().len(), 2);
        assert_eq!(
            group
                .chosen_rows()
                .iter()
                .map(ResultOutputRow::row_id)
                .collect::<BTreeSet<_>>(),
            [
                ResultViewInputRowId::Case(copenhagen),
                ResultViewInputRowId::Case(aarhus),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(closed.counts().output_rows(), ResultViewCount::Exact(2));
    }

    #[test]
    fn municipality_without_variation_is_suppressed_only_after_closure() {
        let (relation_id, question_id, _) = identities("municipality-no-variation");
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"municipality-no-variation-view",
        );
        let spec = ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Case,
            ResultViewGrain::GroupAll,
            names(&["tax_ore"]),
            names(&[]),
            names(&["municipality"]),
            Some(ResultViewHaving::Varies { measure_index: 0 }),
            Some(ResultViewChoice::Optimize {
                cardinality: ExploreChooseCardinality::All,
                direction: ExploreOptimizeDirection::Minimize,
            }),
        )
        .unwrap();
        let first = case(relation_id, "First", 0, 1);
        let second = case(relation_id, "Second", 0, 2);
        let mut projector = FixtureProjector::default();
        let mut builder = ResultViewBuilder::new(spec);
        for (case_id, name) in [(first, "First"), (second, "Second")] {
            builder
                .insert(contribution(
                    &mut projector,
                    view_id,
                    ResultViewInputRowId::Case(case_id),
                    vec![],
                    vec![value(ExploreValue::Int(100_000))],
                    vec![],
                    vec![value(ExploreValue::String(name.to_string()))],
                    vec![100_000],
                ))
                .unwrap();
        }
        let open = builder.snapshot(&mut projector).unwrap();
        let open_group = &open.output().groups().unwrap()[0];
        assert_eq!(
            open_group.disposition(),
            ResultGroupDisposition::Provisional {
                currently_passes_having: false,
            }
        );
        builder.seal_input();
        let closed = builder.finish(&mut projector).unwrap();
        let group = &closed.snapshot().output().groups().unwrap()[0];
        assert_eq!(group.disposition(), ResultGroupDisposition::ExactExcluded);
        assert!(group.chosen_rows().is_empty());
        assert_eq!(
            closed.counts().output_groups(),
            Some(ResultViewCount::Exact(0))
        );
        assert_eq!(closed.counts().output_rows(), ResultViewCount::Exact(0));
    }

    #[test]
    fn choose_one_and_root_are_independent_of_arrival_order() {
        let (relation_id, question_id, _) = identities("canonical-choice");
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"canonical-choice-view",
        );
        let spec = ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Case,
            ResultViewGrain::EachCase,
            names(&[]),
            names(&[]),
            names(&["label"]),
            None,
            Some(ResultViewChoice::Optimize {
                cardinality: ExploreChooseCardinality::One,
                direction: ExploreOptimizeDirection::Maximize,
            }),
        )
        .unwrap();
        let first = case(relation_id, "First", 1, 2);
        let second = case(relation_id, "Second", 2, 3);
        let third = case(relation_id, "Third", 3, 4);
        let mut projector = FixtureProjector::default();
        let rows = [
            contribution(
                &mut projector,
                view_id,
                ResultViewInputRowId::Case(first),
                vec![],
                vec![],
                vec![],
                vec![value(ExploreValue::String("First".to_string()))],
                vec![10],
            ),
            contribution(
                &mut projector,
                view_id,
                ResultViewInputRowId::Case(second),
                vec![],
                vec![],
                vec![],
                vec![value(ExploreValue::String("Second".to_string()))],
                vec![10],
            ),
            contribution(
                &mut projector,
                view_id,
                ResultViewInputRowId::Case(third),
                vec![],
                vec![],
                vec![],
                vec![value(ExploreValue::String("Third".to_string()))],
                vec![9],
            ),
        ];
        let mut left = ResultViewBuilder::new(spec.clone());
        for row in rows.iter().cloned() {
            left.insert(row).unwrap();
        }
        let mut right = ResultViewBuilder::new(spec);
        for row in rows.iter().rev().cloned() {
            right.insert(row).unwrap();
        }
        assert_eq!(
            left.snapshot(&mut projector).unwrap(),
            right.snapshot(&mut projector).unwrap()
        );
        assert_eq!(
            left.snapshot(&mut projector).unwrap().root(),
            right.snapshot(&mut projector).unwrap().root()
        );
        let expected = ResultViewInputRowId::Case(first.min(second));
        assert_eq!(
            left.snapshot(&mut projector)
                .unwrap()
                .output()
                .rows()
                .unwrap()[0]
                .row_id(),
            expected
        );
        left.seal_input();
        right.seal_input();
        assert_eq!(
            left.snapshot(&mut projector).unwrap().root(),
            right.snapshot(&mut projector).unwrap().root()
        );
        assert_eq!(
            left.finish(&mut projector).unwrap().counts().output_rows(),
            ResultViewCount::Exact(1)
        );
    }

    #[test]
    fn pareto_retains_all_nondominated_rows_including_equal_vectors() {
        let (relation_id, question_id, _) = identities("pareto");
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"pareto-view",
        );
        let spec = ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Case,
            ResultViewGrain::EachCase,
            names(&[]),
            names(&[]),
            names(&["plan"]),
            None,
            Some(ResultViewChoice::Pareto {
                directions: vec![
                    ExploreOptimizeDirection::Maximize,
                    ExploreOptimizeDirection::Minimize,
                ]
                .into_boxed_slice(),
            }),
        )
        .unwrap();
        let a = case(relation_id, "A", 0, 1);
        let b = case(relation_id, "B", 0, 2);
        let c = case(relation_id, "C", 0, 3);
        let d = case(relation_id, "D", 0, 4);
        let mut projector = FixtureProjector::default();
        let mut builder = ResultViewBuilder::new(spec);
        for (case_id, label, objectives) in [
            (a, "A", vec![10, 10]),
            (b, "B", vec![9, 8]),
            (c, "C", vec![8, 12]),
            (d, "D", vec![10, 10]),
        ] {
            builder
                .insert(contribution(
                    &mut projector,
                    view_id,
                    ResultViewInputRowId::Case(case_id),
                    vec![],
                    vec![],
                    vec![],
                    vec![value(ExploreValue::String(label.to_string()))],
                    objectives,
                ))
                .unwrap();
        }
        builder.seal_input();
        let closed = builder.finish(&mut projector).unwrap();
        assert_eq!(
            closed
                .snapshot()
                .output()
                .rows()
                .unwrap()
                .iter()
                .map(ResultOutputRow::row_id)
                .collect::<BTreeSet<_>>(),
            [a, b, d]
                .into_iter()
                .map(ResultViewInputRowId::Case)
                .collect()
        );
        assert_eq!(closed.counts().output_rows(), ResultViewCount::Exact(3));
    }

    #[test]
    fn grouped_projection_is_separate_and_rows_are_idempotent() {
        let (relation_id, question_id, _) = identities("validation");
        let grouped_view = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"invalid-grouped-view",
        );
        let grouped_spec = ResultViewSpec::new(
            grouped_view,
            ResultViewInputKind::Case,
            ResultViewGrain::GroupAll,
            names(&[]),
            names(&[]),
            names(&["renamed_or_computed"]),
            None,
            None,
        )
        .unwrap();
        let mut grouped_projector = FixtureProjector {
            rows: BTreeMap::new(),
            group_values: vec![value(ExploreValue::Int(42))].into_boxed_slice(),
        };
        let grouped = ResultViewBuilder::new(grouped_spec)
            .snapshot(&mut grouped_projector)
            .unwrap();
        assert_eq!(
            grouped.output().groups().unwrap()[0].projected_values(),
            Some([value(ExploreValue::Int(42))].as_slice())
        );

        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::Selected(question_id),
            b"valid-each-case-view",
        );
        let spec = ResultViewSpec::new(
            view_id,
            ResultViewInputKind::Case,
            ResultViewGrain::EachCase,
            names(&["measure"]),
            names(&[]),
            names(&["value"]),
            None,
            None,
        )
        .unwrap();
        let case_id = case(relation_id, "Case", 1, 2);
        let mut projector = FixtureProjector::default();
        let row = contribution(
            &mut projector,
            view_id,
            ResultViewInputRowId::Case(case_id),
            vec![],
            vec![value(ExploreValue::Int(1))],
            vec![],
            vec![value(ExploreValue::Int(1))],
            vec![],
        );
        let mut builder = ResultViewBuilder::new(spec);
        assert!(builder.insert(row.clone()).unwrap());
        assert!(!builder.insert(row.clone()).unwrap());
        let changed = contribution(
            &mut projector,
            view_id,
            ResultViewInputRowId::Case(case_id),
            vec![],
            vec![value(ExploreValue::Int(2))],
            vec![],
            vec![value(ExploreValue::Int(2))],
            vec![],
        );
        assert_eq!(
            builder.insert(changed).unwrap_err(),
            ResultViewError::ContributionConflict {
                row_id: ResultViewInputRowId::Case(case_id),
            }
        );
        assert_eq!(
            builder.clone().finish(&mut projector).unwrap_err(),
            ResultViewFinishError::InputFrontierOpen
        );
        builder.seal_input();
        assert!(!builder.insert(row).unwrap());
        let another = contribution(
            &mut projector,
            view_id,
            ResultViewInputRowId::Case(case(relation_id, "Another", 2, 3)),
            vec![],
            vec![value(ExploreValue::Int(3))],
            vec![],
            vec![value(ExploreValue::Int(3))],
            vec![],
        );
        assert_eq!(
            builder.insert(another).unwrap_err(),
            ResultViewError::InputAlreadySealed
        );
    }
}
