//! Canonical, privacy-safe JSON projections of exact Explore evidence.
//!
//! Two artifacts are intentionally distinct:
//!
//! - an observable snapshot is bound to one order-sensitive durable cursor and
//!   ends in one LF byte so it can be emitted as a JSON-lines record; and
//! - a semantic answer omits the cursor and all producer/journal provenance,
//!   so equivalent normalized evidence has identical bytes regardless of
//!   worker arrival order or batching.
//!
//! Both projections omit validation receipts and the optional matching ledger.
//! A future ledger renderer must require its own explicit retention authority;
//! the presence of retained rows inside the reducer is not output authority.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::case_graph::{
    CaseDecisionDag, CaseOpenReason, CaseTerminal, CheckedCardinality, DecisionRef, DecisionRoot,
    DEFAULT_MAX_CASE_RANK_RUNS, DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES,
    DEFAULT_MAX_CASE_RANK_RUN_ARCS, DEFAULT_MAX_CASE_RANK_RUN_AXES,
    DEFAULT_MAX_CASE_RANK_RUN_NODES, DEFAULT_MAX_CASE_RANK_RUN_ORDINAL_INTERVALS,
};
use super::classification_regions::SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1;
use super::exact_stream::{
    transition_semantic_fact_digest_from_parts_v2, ExactCanonicalCaseIdV1,
    ExactConstructibleClassificationV2, ExactCountBoundV1, ExactEvidenceReducer,
    ExactEvidenceSnapshotV1, ExactExtremaAggregateV1, ExactResultAggregateV1,
    ExactTransitionPopulationSnapshotV2, EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_JSON_BYTE_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_SEMANTIC_BYTE_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_VALUE_NODE_LIMIT_V1,
};
use super::report::ExploreSemanticTransitionGraphRequest;
use super::run_stream::{
    CanonicalDigest, ExactCaseSupport, ExploreRunCursor, ExploreRunStream, RunLifecycle,
};
use super::source_events::{SOURCE_PROOF_ADAPTER_LIMITS_V1, SOURCE_PROOF_EXTRACTION_OPTIONS_V1};
use super::source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT;
use super::stream_identity::{
    CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2, CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
};
use super::stream_probe::{
    ExactSourceProbeModeV1, ExactSourceProbePhaseV1, ExactSourceProbeProgressV1,
};
use super::stream_proof::{
    source_proof_candidate_rank_bytes_limit_v1, source_proof_candidate_rank_limit_v1,
    source_proof_closed_region_limit_v1,
};
use super::transition::TransitionSchemaIdentities;
use super::{
    ExploreEnumeratedSource, ExploreExactDomain, ExploreFactValue, ExploreFiniteTypePlan,
    ExploreGeneratorAxisRole, ExploreQueryIr, ExploreValue,
};
use crate::ExplorePolarity;

/// Content-addressed kind for cursor-bearing observable snapshots.
pub(crate) const EXACT_OBSERVABLE_SNAPSHOT_BLOB_KIND_V1: &str = "exact-observable-snapshot-v7";

/// Schema name written into every cursor-bearing observable snapshot.
pub(crate) const EXACT_OBSERVABLE_SNAPSHOT_SCHEMA_V1: &str = "futuruna.explore.snapshot.v7";

/// Content-addressed kind for a bounded observer receipt reporting that one
/// admitted full-snapshot attempt was unavailable because of capacity.
pub(crate) const EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_BLOB_KIND_V1: &str =
    "exact-observable-snapshot-unavailable-v1";

/// Schema name for the bounded, cursor-bearing unavailable receipt above.
pub(crate) const EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_SCHEMA_V1: &str =
    "futuruna.explore.snapshot-unavailable.v1";

/// Content-addressed kind for history-independent semantic answers.
pub(crate) const EXACT_SEMANTIC_ANSWER_BLOB_KIND_V1: &str = "exact-semantic-answer-v6";

/// Internal schema for the evidence-derived answer committed by a terminal
/// payload hash. A public terminal report may wrap this object with additional
/// checked-query presentation metadata.
pub(crate) const EXACT_SEMANTIC_ANSWER_SCHEMA_V1: &str = "futuruna.explore.exact-answer.v6";

const OBSERVABLE_SCHEMA_VERSION_V1: u64 = 7;
const SEMANTIC_ANSWER_SCHEMA_VERSION_V1: u64 = 6;
pub(crate) const MAX_CANONICAL_JSON_BYTES: usize = 96 * 1024 * 1024;
/// Cumulative terminal row budget. The remaining 48 MiB covers both bounded
/// graph objects, answer metadata, projection labels, JSON escaping, and the
/// enclosing writer, so an individually admitted graph cannot consume the
/// entire former metadata reserve.
pub(crate) const MAX_TERMINAL_RESULT_ROW_JSON_BYTES_V1: usize = 48 * 1024 * 1024;
pub(crate) const MAX_PROJECTION_LABELS: usize = 65_536;
pub(crate) const MAX_PROJECTION_LABEL_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_PROJECTION_LABEL_TOTAL_BYTES_V1: usize = 4 * 1024 * 1024;
/// Exact canonical-JSON byte budget for every checked presentation string that
/// snapshot/terminal metadata clones outside the separately bounded value
/// bodies. Repeated occurrences (for example a boundary axis or `having`
/// extrema label) are charged repeatedly because they are serialized twice.
pub(crate) const MAX_PRESENTATION_STRING_JSON_BYTES_V1: usize = 8 * 1024 * 1024;
/// Total occurrences, not unique values. Every axis, fact, named source and
/// projection entry therefore charges retained structural overhead even when
/// its string is empty or repeated. At a conservative 256-byte metadata charge
/// this caps entry structure at 64 MiB inside the 256 MiB view envelope.
pub(crate) const MAX_PRESENTATION_STRING_OCCURRENCES_V1: usize = 262_144;

/// Conservative peak envelope for one admitted snapshot materialization.
///
/// This covers the 64 MiB search-decision-DAG lowerer, 96 MiB outer canonical
/// writer, nested graph/result/configuration JSON limits, validation scratch
/// and reallocation overlap. The current one-worker stream remains in the cold
/// resource phase whose >=2 GiB charge dominates this value. A future
/// calibrated/multiworker publisher must introduce a distinct snapshot charge
/// before it may admit this work in scan mode.
pub(crate) const EXACT_OBSERVABLE_SNAPSHOT_ACCOUNTED_WORKING_SET_V1: u64 = 256 * 1024 * 1024;

/// The unavailable observer receipt is deliberately tiny and independent of
/// query-controlled strings. It can therefore close an observation debt even
/// when an admitted full-snapshot attempt cannot complete.
pub(crate) const EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_JSON_BYTE_LIMIT_V1: usize = 4 * 1024;

/// Atomic publication cap for the canonical nested search decision DAG object.
///
/// The object is rendered and hashed independently before it is embedded in a
/// snapshot. Crossing this bound never emits a graph prefix.
pub(crate) const EXACT_SEARCH_DECISION_DAG_CANONICAL_JSON_BYTE_LIMIT_V1: usize = 8 * 1024 * 1024;

/// Atomic publication cap for the canonical nested semantic transition graph.
///
/// The graph is rendered and hashed independently. Crossing this bound emits
/// only a typed capacity status; no state or transition prefix escapes.
pub(crate) const EXACT_SEMANTIC_TRANSITION_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1: usize =
    8 * 1024 * 1024;

struct ExactPresentationStringBudgetV1 {
    encoded_bytes: usize,
    occurrences: usize,
}

impl ExactPresentationStringBudgetV1 {
    fn new() -> Self {
        Self {
            encoded_bytes: 0,
            occurrences: 0,
        }
    }

    fn charge(&mut self, kind: &str, value: &str) -> Result<(), ExactSnapshotRenderError> {
        self.occurrences = self.occurrences.checked_add(1).ok_or_else(|| {
            ExactSnapshotRenderError::limit("presentation-string occurrence count overflow")
        })?;
        if self.occurrences > MAX_PRESENTATION_STRING_OCCURRENCES_V1 {
            return Err(ExactSnapshotRenderError::limit(format!(
                "checked presentation strings exceed the cumulative {MAX_PRESENTATION_STRING_OCCURRENCES_V1}-occurrence metadata limit"
            )));
        }
        if value.len() > MAX_PROJECTION_LABEL_BYTES {
            return Err(ExactSnapshotRenderError::limit(format!(
                "{kind} exceeds the {MAX_PROJECTION_LABEL_BYTES}-byte presentation-string limit"
            )));
        }
        let encoded = canonical_json_string_encoded_len(value).ok_or_else(|| {
            ExactSnapshotRenderError::limit("presentation-string JSON byte accounting overflow")
        })?;
        self.encoded_bytes = self.encoded_bytes.checked_add(encoded).ok_or_else(|| {
            ExactSnapshotRenderError::limit("presentation-string JSON byte accounting overflow")
        })?;
        if self.encoded_bytes > MAX_PRESENTATION_STRING_JSON_BYTES_V1 {
            return Err(ExactSnapshotRenderError::limit(format!(
                "checked presentation strings exceed the cumulative {MAX_PRESENTATION_STRING_JSON_BYTES_V1}-byte canonical-JSON limit"
            )));
        }
        Ok(())
    }
}

fn canonical_json_string_encoded_len(value: &str) -> Option<usize> {
    value.bytes().try_fold(2_usize, |total, byte| {
        let width = match byte {
            b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
            0x00..=0x1f => 6,
            _ => 1,
        };
        total.checked_add(width)
    })
}

/// Preflight every checked name copied into snapshot/terminal metadata before
/// run creation. This is identity-bound and allocation-free over borrowed IR,
/// so a valid but pathologically named query cannot escape the admitted view
/// working-set envelope before the bounded writer sees it.
pub(crate) fn validate_exact_snapshot_presentation_v1(
    query: &ExploreQueryIr,
) -> Result<(), ExactSnapshotRenderError> {
    let mut budget = ExactPresentationStringBudgetV1::new();
    if let Some(name) = query.query.name.as_deref() {
        budget.charge("Explore query name", name)?;
    }
    for field in &query.query.output.key {
        budget.charge("key projection label", &field.name)?;
    }
    for field in &query.query.output.extrema {
        budget.charge("extrema projection label", &field.name)?;
    }
    for field in &query.query.output.show {
        budget.charge("shown projection label", &field.name)?;
    }
    if let Some(crate::TypedExploreHaving::Varies { extrema_name, .. }) = &query.query.output.having
    {
        budget.charge("having extrema label", extrema_name)?;
    }
    for axis in &query.universe.dimensions {
        budget.charge("axis name", &axis.name)?;
        match &axis.domain {
            ExploreExactDomain::Enumerated {
                source: ExploreEnumeratedSource::NamedList { name },
                ..
            } => budget.charge("named-list source", name)?,
            ExploreExactDomain::Enumerated {
                source: ExploreEnumeratedSource::NamedSet { name },
                ..
            } => budget.charge("named-set source", name)?,
            ExploreExactDomain::Enumerated {
                source: ExploreEnumeratedSource::ExplicitList,
                ..
            }
            | ExploreExactDomain::IntRange { .. }
            | ExploreExactDomain::FiniteType { .. } => {}
        }
    }
    for fact in &query.universe.facts {
        budget.charge("fact name", &fact.name)?;
    }
    if let Some(boundary) = query.boundary_hint() {
        budget.charge("boundary axis", &boundary.axis)?;
    }
    Ok(())
}

/// One fixed resource that can prevent all-or-nothing search decision DAG
/// publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExactSearchDecisionDagPublicationResourceV1 {
    LoweringAxes,
    LoweringRankRuns,
    LoweringNodes,
    LoweringArcs,
    LoweringOrdinalIntervals,
    LoweringAccountedBytes,
    CanonicalJsonBytes,
}

impl ExactSearchDecisionDagPublicationResourceV1 {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::LoweringAxes => "lowering_axes",
            Self::LoweringRankRuns => "lowering_rank_runs",
            Self::LoweringNodes => "lowering_nodes",
            Self::LoweringArcs => "lowering_arcs",
            Self::LoweringOrdinalIntervals => "lowering_ordinal_intervals",
            Self::LoweringAccountedBytes => "lowering_accounted_bytes",
            Self::CanonicalJsonBytes => "canonical_json_bytes",
        }
    }

    pub(crate) const fn fixed_maximum(self) -> usize {
        match self {
            Self::LoweringAxes => DEFAULT_MAX_CASE_RANK_RUN_AXES,
            Self::LoweringRankRuns => DEFAULT_MAX_CASE_RANK_RUNS,
            Self::LoweringNodes => DEFAULT_MAX_CASE_RANK_RUN_NODES,
            Self::LoweringArcs => DEFAULT_MAX_CASE_RANK_RUN_ARCS,
            Self::LoweringOrdinalIntervals => DEFAULT_MAX_CASE_RANK_RUN_ORDINAL_INTERVALS,
            Self::LoweringAccountedBytes => DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES,
            Self::CanonicalJsonBytes => EXACT_SEARCH_DECISION_DAG_CANONICAL_JSON_BYTE_LIMIT_V1,
        }
    }
}

/// Fully prepared disclosure state for the search decision DAG requested by one run.
///
/// `CapacityLimited` is evidence that a complete graph could not be
/// materialized under one fixed schema limit. `required_at_least` is a lower
/// bound, not an invented total. No variant can represent a graph prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExactPreparedSearchDecisionDagPublicationV1 {
    NotRequested,
    Included(CaseDecisionDag),
    CapacityLimited {
        resource: ExactSearchDecisionDagPublicationResourceV1,
        maximum: usize,
        required_at_least: usize,
    },
}

/// Fixed resource governing all-or-none semantic transition graph
/// materialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExactSemanticTransitionGraphPublicationResourceV1 {
    CanonicalJsonBytes,
}

impl ExactSemanticTransitionGraphPublicationResourceV1 {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::CanonicalJsonBytes => "canonical_json_bytes",
        }
    }

    pub(crate) const fn fixed_maximum(self) -> usize {
        match self {
            Self::CanonicalJsonBytes => {
                EXACT_SEMANTIC_TRANSITION_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1
            }
        }
    }
}

/// Fully prepared semantic transition graph publication for one reducer
/// revision. Included bytes are already canonical and bounded. Capacity is a
/// complete publication status, never a graph prefix or finalization failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExactPreparedSemanticTransitionGraphPublicationV1 {
    NotRequested,
    Included {
        canonical_graph_object: Vec<u8>,
        artifact_graph_hash: String,
        state_count: u128,
        transition_count: u128,
        constructible_case_count: u128,
    },
    CapacityLimited {
        resource: ExactSemanticTransitionGraphPublicationResourceV1,
        maximum: usize,
        required_at_least: usize,
    },
}

impl ExactPreparedSemanticTransitionGraphPublicationV1 {
    fn requested(&self) -> bool {
        !matches!(self, Self::NotRequested)
    }

    fn complete_for_answer(&self, populations: ExactTransitionPopulationSnapshotV2) -> bool {
        match self {
            Self::NotRequested | Self::CapacityLimited { .. } => true,
            Self::Included { .. } => {
                populations.u_c.exact.is_some() && populations.u_t.exact.is_some()
            }
        }
    }
}

/// Materialize the authenticated semantic transition relation under its fixed
/// all-or-none JSON cap. The stream owns transition-to-case support; the exact
/// reducer owns canonical edge collision witnesses and global classification
/// fibers. Publication intersects those two authorities instead of retaining
/// a duplicate per-edge support index.
pub(crate) fn prepare_exact_semantic_transition_graph_publication_v1(
    request: ExploreSemanticTransitionGraphRequest,
    axis_cardinalities: &[u128],
    schemas: &TransitionSchemaIdentities,
    exact: &ExactEvidenceReducer,
    stream: &ExploreRunStream,
) -> Result<ExactPreparedSemanticTransitionGraphPublicationV1, ExactSnapshotRenderError> {
    if request == ExploreSemanticTransitionGraphRequest::Omit {
        return Ok(ExactPreparedSemanticTransitionGraphPublicationV1::NotRequested);
    }

    let transition_index = exact.transition_support_index();
    let populations = exact.transition_population_snapshot();
    let declared_cases = axis_cardinalities
        .iter()
        .copied()
        .try_fold(1_u128, u128::checked_mul)
        .ok_or_else(|| {
            ExactSnapshotRenderError::invalid(
                "semantic transition graph axis cardinalities exceed u128::MAX",
            )
        })?;
    if populations.u_d.lower_bound != declared_cases {
        return Err(ExactSnapshotRenderError::invalid(
            "semantic transition graph axes disagree with the reducer case universe",
        ));
    }
    let state_count = transition_index.state_len() as u128;
    let transition_count = transition_index.len() as u128;
    let constructible_case_count = populations.u_c.lower_bound;
    if populations.u_t.lower_bound != transition_count {
        return Err(ExactSnapshotRenderError::invalid(
            "semantic transition index disagrees with reducer transition populations",
        ));
    }

    let canonical_graph_object = match render_canonical_semantic_transition_graph_object_v1(
        axis_cardinalities,
        schemas,
        exact,
        stream,
    ) {
        Ok(bytes) => bytes,
        Err(error) if error.is_capacity_limit() => {
            return Ok(
                ExactPreparedSemanticTransitionGraphPublicationV1::CapacityLimited {
                    resource: ExactSemanticTransitionGraphPublicationResourceV1::CanonicalJsonBytes,
                    maximum: EXACT_SEMANTIC_TRANSITION_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1,
                    required_at_least: EXACT_SEMANTIC_TRANSITION_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1
                        + 1,
                },
            );
        }
        Err(error) => return Err(error),
    };
    let artifact_graph_hash = exact_stream_blob_sha256(&canonical_graph_object);
    Ok(
        ExactPreparedSemanticTransitionGraphPublicationV1::Included {
            canonical_graph_object,
            artifact_graph_hash,
            state_count,
            transition_count,
            constructible_case_count,
        },
    )
}

/// Owned names from the type-checked Explore output projection.
///
/// The labels are retained even when no result group has been observed, so an
/// empty or partial snapshot remains self-describing. Validation repeats the
/// checked-query uniqueness contract at the renderer boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactProjectionLabelsV1 {
    key: Box<[String]>,
    extrema: Box<[String]>,
    shown: Box<[String]>,
}

impl ExactProjectionLabelsV1 {
    pub(crate) fn from_checked_query(
        query: &ExploreQueryIr,
    ) -> Result<Self, ExactSnapshotRenderError> {
        for (kind, count) in [
            ("key", query.query.output.key.len()),
            ("extrema", query.query.output.extrema.len()),
            ("shown", query.query.output.show.len()),
        ] {
            if count > MAX_PROJECTION_LABELS {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "{kind} projection has {count} labels; limit is {MAX_PROJECTION_LABELS}"
                )));
            }
        }
        let mut total_bytes = 0_usize;
        let mut unique = BTreeSet::new();
        let borrowed_labels = query
            .query
            .output
            .key
            .iter()
            .map(|field| ("key", field.name.as_str()))
            .chain(
                query
                    .query
                    .output
                    .extrema
                    .iter()
                    .map(|field| ("extrema", field.name.as_str())),
            )
            .chain(
                query
                    .query
                    .output
                    .show
                    .iter()
                    .map(|field| ("shown", field.name.as_str())),
            );
        for (kind, label) in borrowed_labels {
            if label.is_empty() {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "{kind} projection label must not be empty"
                )));
            }
            if label.len() > MAX_PROJECTION_LABEL_BYTES {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "{kind} projection label `{label}` exceeds {MAX_PROJECTION_LABEL_BYTES} UTF-8 bytes"
                )));
            }
            total_bytes = total_bytes.checked_add(label.len()).ok_or_else(|| {
                ExactSnapshotRenderError::limit(
                    "projection label byte accounting exceeds usize::MAX",
                )
            })?;
            if total_bytes > MAX_PROJECTION_LABEL_TOTAL_BYTES_V1 {
                return Err(ExactSnapshotRenderError::limit(format!(
                    "projection labels exceed the cumulative {MAX_PROJECTION_LABEL_TOTAL_BYTES_V1}-byte snapshot limit"
                )));
            }
            if !unique.insert(label) {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "projection label `{label}` occurs more than once"
                )));
            }
        }
        let labels = Self {
            key: query
                .query
                .output
                .key
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            extrema: query
                .query
                .output
                .extrema
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            shown: query
                .query
                .output
                .show
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        };
        labels.validate()?;
        Ok(labels)
    }

    fn validate(&self) -> Result<(), ExactSnapshotRenderError> {
        let mut unique = BTreeSet::new();
        for (kind, labels) in [
            ("key", self.key.as_ref()),
            ("extrema", self.extrema.as_ref()),
            ("shown", self.shown.as_ref()),
        ] {
            if labels.len() > MAX_PROJECTION_LABELS {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "{kind} projection has {} labels; limit is {MAX_PROJECTION_LABELS}",
                    labels.len()
                )));
            }
            for label in labels {
                if label.is_empty() {
                    return Err(ExactSnapshotRenderError::invalid(format!(
                        "{kind} projection label must not be empty"
                    )));
                }
                if label.len() > MAX_PROJECTION_LABEL_BYTES {
                    return Err(ExactSnapshotRenderError::invalid(format!(
                        "{kind} projection label `{label}` exceeds {MAX_PROJECTION_LABEL_BYTES} UTF-8 bytes"
                    )));
                }
                if !unique.insert(label.as_str()) {
                    return Err(ExactSnapshotRenderError::invalid(format!(
                        "projection label `{label}` occurs more than once"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Checked post-aggregation selection for the public result view.
///
/// The exact reducer deliberately retains every raw key group. Applying this
/// filter only while rendering keeps evidence validation independent of the
/// requested view and lets a partial stream emit a group as soon as observed
/// extrema prove that it must qualify in every continuation.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ExactGroupFilterV1 {
    All,
    Varies {
        extrema_index: usize,
        extrema_name: String,
    },
}

impl ExactGroupFilterV1 {
    fn from_checked_query(
        query: &ExploreQueryIr,
        labels: &ExactProjectionLabelsV1,
    ) -> Result<Self, ExactSnapshotRenderError> {
        let filter = match &query.query.output.having {
            None => Self::All,
            Some(crate::TypedExploreHaving::Varies {
                extrema_name,
                extrema_index,
                ..
            }) => Self::Varies {
                extrema_index: *extrema_index,
                extrema_name: extrema_name.clone(),
            },
        };
        filter.validate(labels)?;
        Ok(filter)
    }

    fn validate(&self, labels: &ExactProjectionLabelsV1) -> Result<(), ExactSnapshotRenderError> {
        let Self::Varies {
            extrema_index,
            extrema_name,
        } = self
        else {
            return Ok(());
        };
        let Some(checked_name) = labels.extrema.get(*extrema_index) else {
            return Err(ExactSnapshotRenderError::invalid(format!(
                "checked varies filter index {extrema_index} is outside {} extrema fields",
                labels.extrema.len()
            )));
        };
        if checked_name != extrema_name {
            return Err(ExactSnapshotRenderError::invalid(format!(
                "checked varies filter names `{extrema_name}` at index {extrema_index}, but the projection names `{checked_name}`"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExactObservableDomainV1 {
    Enumerated {
        cardinality: u128,
        source: ExactObservableEnumeratedSourceV1,
        values: Option<Box<[ExploreValue]>>,
        omission_reason: Option<ExactObservableValueOmissionV1>,
    },
    IntRange {
        start: i64,
        end_exclusive: i64,
        cardinality: u128,
    },
    FiniteType {
        cardinality: u128,
        inhabitants: Option<Box<[ExploreValue]>>,
        omission_reason: Option<ExactObservableValueOmissionV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExactObservableEnumeratedSourceV1 {
    ExplicitList,
    NamedList(String),
    NamedSet(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactObservableAxisV1 {
    name: String,
    bound_index: usize,
    role: ExploreGeneratorAxisRole,
    role_field_index: usize,
    domain: ExactObservableDomainV1,
}

fn generator_axis_role_name(role: ExploreGeneratorAxisRole) -> &'static str {
    match role {
        ExploreGeneratorAxisRole::Context => "context",
        ExploreGeneratorAxisRole::Before => "before",
        ExploreGeneratorAxisRole::AfterIndependent => "after_independent",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactObservableFixedFactV1 {
    name: String,
    bound_index: usize,
    role: ExploreGeneratorAxisRole,
    role_field_index: usize,
    value: Option<ExploreValue>,
    omission_reason: Option<ExactObservableValueOmissionV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactObservableDerivedFactV1 {
    name: String,
    bound_index: usize,
    role: ExploreGeneratorAxisRole,
    role_field_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExactObservableValueOmissionV1 {
    NodeLimit,
    SemanticByteLimit,
    NodeAndSemanticByteLimit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExactObservableValueCostV1 {
    nodes: usize,
    semantic_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactObservableValueBudgetV1 {
    remaining_nodes: usize,
    remaining_semantic_bytes: usize,
}

impl ExactObservableValueBudgetV1 {
    fn full() -> Self {
        Self {
            remaining_nodes: CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2,
            remaining_semantic_bytes: CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
        }
    }

    fn charge(
        &mut self,
        cost: ExactObservableValueCostV1,
    ) -> Result<(), ExactObservableValueOmissionV1> {
        let nodes_exceeded = cost.nodes > self.remaining_nodes;
        let bytes_exceeded = cost.semantic_bytes > self.remaining_semantic_bytes;
        match (nodes_exceeded, bytes_exceeded) {
            (true, true) => Err(ExactObservableValueOmissionV1::NodeAndSemanticByteLimit),
            (true, false) => Err(ExactObservableValueOmissionV1::NodeLimit),
            (false, true) => Err(ExactObservableValueOmissionV1::SemanticByteLimit),
            (false, false) => {
                self.remaining_nodes -= cost.nodes;
                self.remaining_semantic_bytes -= cost.semantic_bytes;
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactObservableBoundaryV1 {
    axis: String,
    axis_dimension_index: usize,
    step: i64,
    requires_both_endpoints_in_domain: bool,
}

/// Checked configuration manifest carried by every saved snapshot. Domain
/// hashes remain the machine identity; the bounded shape is repeated here so
/// a probe artifact can be inspected without guessing which source range and
/// fixed facts it described. Values are disclosed only while the one global
/// recursive node-and-semantic-byte budget has room; every omission is named.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactObservableConfigurationV1 {
    program_hash: CanonicalDigest,
    analysis_program_hash: CanonicalDigest,
    query_hash: CanonicalDigest,
    domain_hash: CanonicalDigest,
    report_request_hash: CanonicalDigest,
    probe_plan_hash: CanonicalDigest,
    evaluator_contract_hash: CanonicalDigest,
    mechanism_observation_hash: CanonicalDigest,
    retention_authorization_hash: CanonicalDigest,
    universe_case_count: u128,
    axes: Box<[ExactObservableAxisV1]>,
    fixed_facts: Box<[ExactObservableFixedFactV1]>,
    derived_facts: Box<[ExactObservableDerivedFactV1]>,
    constraint_count: u128,
    boundary: Option<ExactObservableBoundaryV1>,
}

impl ExactObservableConfigurationV1 {
    fn from_checked_stream(
        stream: &ExploreRunStream,
        query: &ExploreQueryIr,
    ) -> Result<Self, ExactSnapshotRenderError> {
        validate_exact_snapshot_presentation_v1(query)?;
        let expected_cardinalities = stream.header().case_universe().axis_cardinalities();
        if query.universe.dimensions.len() != expected_cardinalities.len() {
            return Err(ExactSnapshotRenderError::invalid(
                "checked snapshot dimension count disagrees with the committed CaseId universe",
            ));
        }
        let mut value_budget = ExactObservableValueBudgetV1::full();
        let axes = query
            .universe
            .dimensions
            .iter()
            .zip(expected_cardinalities.iter().copied())
            .map(|(axis, committed_cardinality)| {
                let cardinality = axis.domain.cardinality().exact().ok_or_else(|| {
                    ExactSnapshotRenderError::invalid(format!(
                        "checked axis `{}` cardinality exceeds u128::MAX",
                        axis.name
                    ))
                })?;
                if cardinality != committed_cardinality {
                    return Err(ExactSnapshotRenderError::invalid(format!(
                        "checked axis `{}` cardinality disagrees with the committed CaseId universe",
                        axis.name
                    )));
                }
                let domain = match &axis.domain {
                    ExploreExactDomain::Enumerated { values, source } => {
                        let cost = observable_values_cost(values);
                        let (disclosed, omission_reason) = match value_budget.charge(cost) {
                            Ok(()) => (Some(values.clone().into_boxed_slice()), None),
                            Err(reason) => (None, Some(reason)),
                        };
                        let source = match source {
                            ExploreEnumeratedSource::ExplicitList => {
                                ExactObservableEnumeratedSourceV1::ExplicitList
                            }
                            ExploreEnumeratedSource::NamedList { name } => {
                                ExactObservableEnumeratedSourceV1::NamedList(name.clone())
                            }
                            ExploreEnumeratedSource::NamedSet { name } => {
                                ExactObservableEnumeratedSourceV1::NamedSet(name.clone())
                            }
                        };
                        ExactObservableDomainV1::Enumerated {
                            cardinality,
                            source,
                            values: disclosed,
                            omission_reason,
                        }
                    }
                    ExploreExactDomain::IntRange {
                        start,
                        end_exclusive,
                        ..
                    } => ExactObservableDomainV1::IntRange {
                        start: *start,
                        end_exclusive: *end_exclusive,
                        cardinality,
                    },
                    ExploreExactDomain::FiniteType { plan, .. } => {
                        let cost = observable_finite_plan_cost(plan)?;
                        let (inhabitants, omission_reason) = match value_budget.charge(cost) {
                            Ok(()) => {
                                // Decode only the already budgeted canonical
                                // inhabitants. `ExploreFiniteTypePlan::enumerate`
                                // builds Cartesian intermediates and can be much
                                // larger than the final inhabitant set when a
                                // later component is empty. Rank decoding keeps
                                // allocation proportional to the disclosed
                                // values and their precomputed recursive cost.
                                let cardinality = usize::try_from(cardinality).map_err(|_| {
                                    ExactSnapshotRenderError::invalid(format!(
                                        "checked finite axis `{}` cardinality does not fit its bounded display",
                                        axis.name
                                    ))
                                })?;
                                let values = (0..cardinality)
                                    .map(|ordinal| {
                                        plan.exact_value_at(ordinal as u128)
                                            .map_err(ExactSnapshotRenderError::invalid)
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                                if observable_values_cost(&values) != cost {
                                    return Err(ExactSnapshotRenderError::invalid(format!(
                                        "checked finite axis `{}` materialization cost disagrees with its bounded plan cost",
                                        axis.name
                                    )));
                                }
                                (Some(values.into_boxed_slice()), None)
                            }
                            Err(reason) => (None, Some(reason)),
                        };
                        ExactObservableDomainV1::FiniteType {
                            cardinality,
                            inhabitants,
                            omission_reason,
                        }
                    }
                };
                Ok(ExactObservableAxisV1 {
                    name: axis.name.clone(),
                    bound_index: axis.bound_index,
                    role: axis.role,
                    role_field_index: axis.role_field_index,
                    domain,
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let mut fixed_facts = Vec::new();
        for fact in query.universe.facts.iter() {
            let ExploreFactValue::Fixed(value) = &fact.value else {
                continue;
            };
            let cost = observable_value_cost(value);
            let (value, omission_reason) = match value_budget.charge(cost) {
                Ok(()) => (Some(value.clone()), None),
                Err(reason) => (None, Some(reason)),
            };
            fixed_facts.push(ExactObservableFixedFactV1 {
                name: fact.name.clone(),
                bound_index: fact.bound_index,
                role: fact.role,
                role_field_index: fact.role_field_index,
                value,
                omission_reason,
            });
        }
        let fixed_facts = fixed_facts.into_boxed_slice();
        let derived_facts = query
            .universe
            .facts
            .iter()
            .filter_map(|fact| match &fact.value {
                ExploreFactValue::Derived { .. } => Some(ExactObservableDerivedFactV1 {
                    name: fact.name.clone(),
                    bound_index: fact.bound_index,
                    role: fact.role,
                    role_field_index: fact.role_field_index,
                }),
                ExploreFactValue::Fixed(_) => None,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let boundary = query
            .boundary_hint()
            .map(|boundary| ExactObservableBoundaryV1 {
                axis: boundary.axis.clone(),
                axis_dimension_index: boundary.axis_dimension_index,
                step: boundary.step,
                requires_both_endpoints_in_domain: boundary.requires_both_endpoints_in_domain,
            });
        let identity = stream.header().identity();
        Ok(Self {
            program_hash: identity.program_hash(),
            analysis_program_hash: identity.analysis_program_hash(),
            query_hash: identity.query_hash(),
            domain_hash: identity.domain_hash(),
            report_request_hash: identity.report_request_hash(),
            probe_plan_hash: identity.probe_plan_hash(),
            evaluator_contract_hash: identity.evaluator_contract_hash(),
            mechanism_observation_hash: identity.mechanism_observation_hash(),
            retention_authorization_hash: identity.retention_authorization_hash(),
            universe_case_count: stream.header().case_universe().case_count(),
            axes,
            fixed_facts,
            derived_facts,
            constraint_count: query.universe.constraints.len() as u128,
            boundary,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactObservableFinitePlanCostV1 {
    cardinality: u128,
    values: ExactObservableValueCostV1,
}

fn observable_values_cost(values: &[ExploreValue]) -> ExactObservableValueCostV1 {
    let mut cost = ExactObservableValueCostV1::default();
    for value in values {
        observable_value_cost_into(&mut cost, value);
        if observable_value_cost_fully_exceeded(cost) {
            break;
        }
    }
    cost
}

fn observable_value_cost(value: &ExploreValue) -> ExactObservableValueCostV1 {
    observable_values_cost(std::slice::from_ref(value))
}

fn observable_value_cost_into(cost: &mut ExactObservableValueCostV1, value: &ExploreValue) {
    observable_cost_add(
        &mut cost.nodes,
        1,
        CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2,
    );
    observable_cost_add(
        &mut cost.semantic_bytes,
        1,
        CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
    );
    match value {
        ExploreValue::Int(_) | ExploreValue::FloatBits(_) => observable_cost_add(
            &mut cost.semantic_bytes,
            8,
            CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
        ),
        ExploreValue::String(value) => observable_cost_add(
            &mut cost.semantic_bytes,
            value.len().checked_add(4).unwrap_or(usize::MAX),
            CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
        ),
        ExploreValue::Character(_) => observable_cost_add(
            &mut cost.semantic_bytes,
            4,
            CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
        ),
        ExploreValue::Boolean(_) => observable_cost_add(
            &mut cost.semantic_bytes,
            1,
            CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
        ),
        ExploreValue::Unit => {}
        ExploreValue::List(values) | ExploreValue::Set(values) | ExploreValue::Tuple(values) => {
            observable_cost_add(
                &mut cost.semantic_bytes,
                4,
                CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
            );
            for child in values {
                observable_value_cost_into(cost, child);
                if observable_value_cost_fully_exceeded(*cost) {
                    break;
                }
            }
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            fields,
            ..
        } => {
            for length in [type_name.len(), variant.len()] {
                observable_cost_add(
                    &mut cost.semantic_bytes,
                    length.checked_add(4).unwrap_or(usize::MAX),
                    CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
                );
            }
            observable_cost_add(
                &mut cost.semantic_bytes,
                5,
                CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
            );
            for (name, child) in fields {
                observable_cost_add(
                    &mut cost.semantic_bytes,
                    name.len().checked_add(4).unwrap_or(usize::MAX),
                    CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
                );
                observable_value_cost_into(cost, child);
                if observable_value_cost_fully_exceeded(*cost) {
                    break;
                }
            }
        }
    }
}

fn observable_value_cost_fully_exceeded(cost: ExactObservableValueCostV1) -> bool {
    cost.nodes > CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2
        || cost.semantic_bytes > CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2
}

fn observable_cost_add(target: &mut usize, amount: usize, limit: usize) {
    if *target > limit {
        return;
    }
    *target = target
        .checked_add(amount)
        .filter(|sum| *sum <= limit)
        .unwrap_or(limit + 1);
}

fn observable_finite_plan_cost(
    plan: &ExploreFiniteTypePlan,
) -> Result<ExactObservableValueCostV1, ExactSnapshotRenderError> {
    Ok(observable_finite_plan_summary(plan)?.values)
}

fn observable_finite_plan_summary(
    plan: &ExploreFiniteTypePlan,
) -> Result<ExactObservableFinitePlanCostV1, ExactSnapshotRenderError> {
    match plan {
        ExploreFiniteTypePlan::Unit => Ok(ExactObservableFinitePlanCostV1 {
            cardinality: 1,
            values: ExactObservableValueCostV1 {
                nodes: 1,
                semantic_bytes: 1,
            },
        }),
        ExploreFiniteTypePlan::Bool => Ok(ExactObservableFinitePlanCostV1 {
            cardinality: 2,
            values: ExactObservableValueCostV1 {
                nodes: 2,
                semantic_bytes: 4,
            },
        }),
        ExploreFiniteTypePlan::Tuple {
            elements,
            cardinality,
        } => {
            let cardinality = cardinality.exact().ok_or_else(|| {
                ExactSnapshotRenderError::invalid("finite tuple cardinality exceeds u128::MAX")
            })?;
            if cardinality > CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2 as u128 {
                return Ok(ExactObservableFinitePlanCostV1 {
                    cardinality,
                    values: ExactObservableValueCostV1 {
                        nodes: CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2 + 1,
                        semantic_bytes: 0,
                    },
                });
            }
            let mut values = ExactObservableValueCostV1 {
                nodes: observable_capped_u128(
                    cardinality,
                    CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2,
                ),
                semantic_bytes: observable_capped_scaled(
                    5,
                    cardinality,
                    CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
                ),
            };
            for element in elements {
                let child = observable_finite_plan_summary(element)?;
                let repetitions =
                    observable_finite_repetitions(cardinality, child.cardinality, "tuple element")?;
                observable_cost_add_scaled(
                    &mut values.nodes,
                    child.values.nodes,
                    repetitions,
                    CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2,
                );
                observable_cost_add_scaled(
                    &mut values.semantic_bytes,
                    child.values.semantic_bytes,
                    repetitions,
                    CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
                );
                if observable_value_cost_fully_exceeded(values) {
                    return Ok(ExactObservableFinitePlanCostV1 {
                        cardinality,
                        values,
                    });
                }
            }
            Ok(ExactObservableFinitePlanCostV1 {
                cardinality,
                values,
            })
        }
        ExploreFiniteTypePlan::Sum {
            type_name,
            variants,
            cardinality,
        } => {
            let cardinality = cardinality.exact().ok_or_else(|| {
                ExactSnapshotRenderError::invalid("finite sum cardinality exceeds u128::MAX")
            })?;
            if cardinality > CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2 as u128 {
                return Ok(ExactObservableFinitePlanCostV1 {
                    cardinality,
                    values: ExactObservableValueCostV1 {
                        nodes: CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2 + 1,
                        semantic_bytes: 0,
                    },
                });
            }
            let mut accumulated_cardinality = 0_u128;
            let mut values = ExactObservableValueCostV1::default();
            for variant in variants {
                let variant_cardinality = variant
                    .fields
                    .iter()
                    .try_fold(1_u128, |product, field| {
                        field
                            .plan
                            .cardinality()
                            .exact()
                            .and_then(|cardinality| product.checked_mul(cardinality))
                    })
                    .ok_or_else(|| {
                        ExactSnapshotRenderError::invalid(
                            "finite constructor cardinality exceeds u128::MAX",
                        )
                    })?;
                accumulated_cardinality = accumulated_cardinality
                    .checked_add(variant_cardinality)
                    .ok_or_else(|| {
                        ExactSnapshotRenderError::invalid(
                            "finite sum cardinality exceeds u128::MAX",
                        )
                    })?;
                observable_cost_add_scaled(
                    &mut values.nodes,
                    1,
                    variant_cardinality,
                    CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2,
                );
                let mut constructor_bytes = 14_usize;
                constructor_bytes = constructor_bytes
                    .checked_add(type_name.len())
                    .and_then(|value| value.checked_add(variant.name.len()))
                    .unwrap_or(usize::MAX);
                for field in variant.fields.iter() {
                    constructor_bytes = constructor_bytes
                        .checked_add(4)
                        .and_then(|value| value.checked_add(field.name.len()))
                        .unwrap_or(usize::MAX);
                }
                observable_cost_add_scaled(
                    &mut values.semantic_bytes,
                    constructor_bytes,
                    variant_cardinality,
                    CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
                );
                if observable_value_cost_fully_exceeded(values) {
                    return Ok(ExactObservableFinitePlanCostV1 {
                        cardinality,
                        values,
                    });
                }
                for field in variant.fields.iter() {
                    let child = observable_finite_plan_summary(&field.plan)?;
                    let repetitions = observable_finite_repetitions(
                        variant_cardinality,
                        child.cardinality,
                        "constructor field",
                    )?;
                    observable_cost_add_scaled(
                        &mut values.nodes,
                        child.values.nodes,
                        repetitions,
                        CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2,
                    );
                    observable_cost_add_scaled(
                        &mut values.semantic_bytes,
                        child.values.semantic_bytes,
                        repetitions,
                        CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2,
                    );
                    if observable_value_cost_fully_exceeded(values) {
                        return Ok(ExactObservableFinitePlanCostV1 {
                            cardinality,
                            values,
                        });
                    }
                }
            }
            if accumulated_cardinality != cardinality {
                return Err(ExactSnapshotRenderError::invalid(
                    "finite sum plan cardinality disagrees with its variants",
                ));
            }
            Ok(ExactObservableFinitePlanCostV1 {
                cardinality,
                values,
            })
        }
    }
}

fn observable_finite_repetitions(
    whole: u128,
    part: u128,
    label: &str,
) -> Result<u128, ExactSnapshotRenderError> {
    if whole == 0 {
        return Ok(0);
    }
    if part == 0 || whole % part != 0 {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "finite {label} cardinality does not divide its containing plan"
        )));
    }
    Ok(whole / part)
}

fn observable_capped_u128(value: u128, limit: usize) -> usize {
    if value <= limit as u128 {
        value as usize
    } else {
        limit + 1
    }
}

fn observable_capped_scaled(value: usize, factor: u128, limit: usize) -> usize {
    if value == 0 || factor == 0 {
        return 0;
    }
    let value = value as u128;
    value
        .checked_mul(factor)
        .filter(|product| *product <= limit as u128)
        .map(|product| product as usize)
        .unwrap_or(limit + 1)
}

fn observable_cost_add_scaled(target: &mut usize, value: usize, factor: u128, limit: usize) {
    observable_cost_add(
        target,
        observable_capped_scaled(value, factor, limit),
        limit,
    );
}

/// Immutable metadata that may affect a normalized semantic answer.
///
/// `answer_scope_hash` commits the checked program, query, domain, report,
/// disclosure policy and complete universe through the pre-probe run header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactSemanticAnswerMetadataV1 {
    schema_digest: CanonicalDigest,
    answer_scope_hash: CanonicalDigest,
    query_name: Option<String>,
    polarity: ExplorePolarity,
    projection_labels: ExactProjectionLabelsV1,
    group_filter: ExactGroupFilterV1,
    required_open_case_count: u128,
    required_open_obligation_count: u128,
}

impl ExactSemanticAnswerMetadataV1 {
    pub(crate) fn from_checked_stream(
        stream: &ExploreRunStream,
        query: &ExploreQueryIr,
    ) -> Result<Self, ExactSnapshotRenderError> {
        validate_exact_snapshot_presentation_v1(query)?;
        let projection_labels = ExactProjectionLabelsV1::from_checked_query(query)?;
        let group_filter = ExactGroupFilterV1::from_checked_query(query, &projection_labels)?;
        if query
            .query
            .name
            .as_ref()
            .is_some_and(|name| name.is_empty() || name.len() > MAX_PROJECTION_LABEL_BYTES)
        {
            return Err(ExactSnapshotRenderError::invalid(
                "checked Explore query name is empty or exceeds the presentation byte limit",
            ));
        }
        Ok(Self {
            schema_digest: stream.header().identity().schemas().terminal_result(),
            answer_scope_hash: stream.header().answer_scope_hash(),
            query_name: query.query.name.clone(),
            polarity: query.query.polarity,
            projection_labels,
            group_filter,
            required_open_case_count: stream.frontier().open_cases().case_count(),
            required_open_obligation_count: stream.frontier().open_obligations().len() as u128,
        })
    }
}

/// Cursor metadata for one observable point in a durable Explore stream.
///
/// Writer identity, lease identity and validation provenance are intentionally
/// absent. They remain in the private run store and ordered journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactObservableSnapshotMetadataV1 {
    schema_digest: CanonicalDigest,
    configuration: ExactObservableConfigurationV1,
    semantic_answer: ExactSemanticAnswerMetadataV1,
    cursor: ExploreRunCursor,
    probe_progress: ExactSourceProbeProgressV1,
    phase: ExactObservablePhaseV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExactObservablePhaseV1 {
    Probes,
    CaseSearch,
    Finalization,
    Complete,
}

impl ExactObservableSnapshotMetadataV1 {
    /// Build replay-reconstructable metadata for a content-addressed snapshot
    /// blob at the current pre-publication cursor. Invocation-local stop and
    /// final paused cursor belong to the surrounding invocation receipt.
    pub(crate) fn from_checked_stream(
        stream: &ExploreRunStream,
        query: &ExploreQueryIr,
        probe_progress: ExactSourceProbeProgressV1,
    ) -> Result<Self, ExactSnapshotRenderError> {
        let phase = if !probe_progress.complete() {
            ExactObservablePhaseV1::Probes
        } else if !stream.frontier().open_cases().is_empty() {
            ExactObservablePhaseV1::CaseSearch
        } else if !stream.frontier().open_obligations().is_empty() {
            ExactObservablePhaseV1::Finalization
        } else {
            ExactObservablePhaseV1::Complete
        };
        Ok(Self {
            schema_digest: stream.header().identity().schemas().snapshot(),
            configuration: ExactObservableConfigurationV1::from_checked_stream(stream, query)?,
            semantic_answer: ExactSemanticAnswerMetadataV1::from_checked_stream(stream, query)?,
            cursor: stream.cursor(),
            probe_progress,
            phase,
        })
    }

    pub(crate) fn semantic_answer(&self) -> &ExactSemanticAnswerMetadataV1 {
        &self.semantic_answer
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ExactSnapshotRenderError {
    message: Box<str>,
    capacity_limit: bool,
}

impl ExactSnapshotRenderError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into().into_boxed_str(),
            capacity_limit: false,
        }
    }

    fn limit(message: impl Into<String>) -> Self {
        Self {
            message: message.into().into_boxed_str(),
            capacity_limit: true,
        }
    }

    pub(crate) const fn is_capacity_limit(&self) -> bool {
        self.capacity_limit
    }
}

impl fmt::Display for ExactSnapshotRenderError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.message)
    }
}

impl Error for ExactSnapshotRenderError {}

/// Render one canonical JSON-lines snapshot, including exactly one trailing LF.
///
/// The semantic `answer` member is arrival-order independent. The containing
/// document intentionally is not: it commits the exact durable observation
/// point through sequence, journal head and evidence root.
pub(crate) fn render_exact_observable_snapshot_json_line_v1(
    metadata: &ExactObservableSnapshotMetadataV1,
    snapshot: &ExactEvidenceSnapshotV1,
    search_decision_dag_publication: &ExactPreparedSearchDecisionDagPublicationV1,
    semantic_transition_graph_publication: &ExactPreparedSemanticTransitionGraphPublicationV1,
) -> Result<Vec<u8>, ExactSnapshotRenderError> {
    let prepared_results = validate_snapshot(
        snapshot,
        &metadata.semantic_answer,
        ExactResultRenderModeV1::ObservablePreview,
    )?;
    let prepared_search_decision_dag = prepare_search_decision_dag_publication(
        search_decision_dag_publication,
        snapshot,
        ExactResultRenderModeV1::ObservablePreview,
    )?;
    validate_semantic_transition_graph_publication(
        semantic_transition_graph_publication,
        snapshot,
    )?;
    let mut writer = CanonicalJsonWriter::new();
    writer.raw(b"{")?;
    writer.member_string("schema", EXACT_OBSERVABLE_SNAPSHOT_SCHEMA_V1)?;
    writer.raw(b",")?;
    writer.member_u64("schema_version", OBSERVABLE_SCHEMA_VERSION_V1)?;
    writer.raw(b",")?;
    writer.member_string("schema_digest", &metadata.schema_digest.to_lowercase_hex())?;
    writer.raw(b",\"run\":{")?;
    writer.member_string("run_id", &metadata.cursor.run_id().to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_decimal("sequence", metadata.cursor.sequence())?;
    writer.raw(b",")?;
    writer.member_string(
        "journal_head",
        &metadata.cursor.journal_head().to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "evidence_root",
        &metadata.cursor.evidence_root().to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string("lifecycle", lifecycle_name(metadata.cursor.lifecycle()))?;
    writer.raw(b",")?;
    writer.member_string("phase", observable_phase_name(metadata.phase))?;
    writer.raw(b",")?;
    writer.member_bool(
        "probe_milestone_complete",
        metadata.probe_progress.complete(),
    )?;
    // A published checkpoint describes the running pre-publication cursor.
    // Invocation stop and final paused cursor live in the outer invocation
    // receipt, so the schema-stable field is necessarily null here.
    writer.raw(b",\"pause_reason\":null")?;
    writer.raw(b",")?;
    writer.member_optional_decimal(
        "last_coverage_epoch",
        metadata
            .cursor
            .last_coverage_epoch()
            .map(|epoch| epoch.get()),
    )?;
    writer.raw(b"},\"probe\":")?;
    write_probe_progress(&mut writer, metadata.probe_progress)?;
    writer.raw(b",\"invocation_stop\":null")?;
    writer.raw(b",\"configuration\":")?;
    write_observable_configuration(&mut writer, &metadata.configuration)?;
    writer.raw(b",\"answer\":")?;
    write_semantic_answer_object(
        &mut writer,
        &metadata.semantic_answer,
        snapshot,
        &prepared_results,
        &prepared_search_decision_dag,
        semantic_transition_graph_publication,
    )?;
    writer.raw(b"}\n")?;
    Ok(writer.finish())
}

/// Render the durable bounded alternative to a full observable snapshot.
///
/// This is not a partial semantic snapshot: it discloses no configuration,
/// result rows, or graph prefix. It merely authenticates that the view at this
/// exact cursor was unavailable during its admitted publication attempt, so
/// replay can distinguish a serviced observer boundary from a transiently
/// deferred one. It does not claim that a later attempt can never fit.
pub(crate) fn render_exact_observable_snapshot_unavailable_json_line_v1(
    stream: &ExploreRunStream,
    probe_milestone_complete: bool,
    closed_case_count: u128,
) -> Result<Vec<u8>, ExactSnapshotRenderError> {
    let cursor = stream.cursor();
    if cursor.lifecycle() != RunLifecycle::Running {
        return Err(ExactSnapshotRenderError::invalid(
            "snapshot-unavailable receipt requires a running pre-publication cursor",
        ));
    }
    let mut writer = CanonicalJsonWriter::with_max_bytes(
        EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_JSON_BYTE_LIMIT_V1,
    );
    writer.raw(b"{")?;
    writer.member_string("schema", EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_SCHEMA_V1)?;
    writer.raw(b",")?;
    writer.member_u64("schema_version", 1)?;
    writer.raw(b",")?;
    writer.member_string(
        "schema_digest",
        &stream
            .header()
            .identity()
            .schemas()
            .snapshot()
            .to_lowercase_hex(),
    )?;
    writer.raw(b",\"run\":{")?;
    writer.member_string("run_id", &cursor.run_id().to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_decimal("sequence", cursor.sequence())?;
    writer.raw(b",")?;
    writer.member_string("journal_head", &cursor.journal_head().to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string("evidence_root", &cursor.evidence_root().to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string("lifecycle", lifecycle_name(cursor.lifecycle()))?;
    writer.raw(b",")?;
    writer.member_optional_decimal(
        "last_coverage_epoch",
        cursor.last_coverage_epoch().map(|epoch| epoch.get()),
    )?;
    writer.raw(b"},\"snapshot\":{")?;
    writer.member_string("status", "unavailable")?;
    writer.raw(b",\"reason\":{")?;
    writer.member_string("kind", "capacity")?;
    writer.raw(b"}},\"progress\":{")?;
    writer.member_bool("probe_milestone_complete", probe_milestone_complete)?;
    writer.raw(b",")?;
    writer.member_decimal("closed_case_count", closed_case_count)?;
    writer.raw(b"}}\n")?;
    Ok(writer.finish())
}

/// Render the canonical history-independent semantic answer.
///
/// These bytes contain no journal cursor, worker identity, receipt, timing,
/// resource sample or retained case ledger. They are suitable input to
/// `TerminalPayloadHash::from_canonical_semantic_payload`.
///
/// Unlike cursor-bearing pause snapshots, this path requires every raw group.
/// A terminal answer larger than the canonical single-blob limit still needs
/// a future chunked publication frontier; it is never silently truncated.
pub(crate) fn render_exact_semantic_answer_json_v1(
    metadata: &ExactSemanticAnswerMetadataV1,
    snapshot: &ExactEvidenceSnapshotV1,
    search_decision_dag_publication: &ExactPreparedSearchDecisionDagPublicationV1,
    semantic_transition_graph_publication: &ExactPreparedSemanticTransitionGraphPublicationV1,
) -> Result<Vec<u8>, ExactSnapshotRenderError> {
    let prepared_results =
        validate_snapshot(snapshot, metadata, ExactResultRenderModeV1::FullPublication)?;
    let prepared_search_decision_dag = prepare_search_decision_dag_publication(
        search_decision_dag_publication,
        snapshot,
        ExactResultRenderModeV1::FullPublication,
    )?;
    validate_semantic_transition_graph_publication(
        semantic_transition_graph_publication,
        snapshot,
    )?;
    let mut writer = CanonicalJsonWriter::new();
    write_semantic_answer_object(
        &mut writer,
        metadata,
        snapshot,
        &prepared_results,
        &prepared_search_decision_dag,
        semantic_transition_graph_publication,
    )?;
    Ok(writer.finish())
}

/// Raw SHA-256 used by the immutable run-store blob namespace.
///
/// Domain-separated semantic terminal commitments remain the responsibility of
/// `TerminalPayloadHash`; this helper only names exact stored bytes.
pub(crate) fn exact_stream_blob_sha256(canonical_bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(canonical_bytes).into();
    lowercase_hex(&digest)
}

fn write_probe_progress(
    writer: &mut CanonicalJsonWriter,
    progress: ExactSourceProbeProgressV1,
) -> Result<(), ExactSnapshotRenderError> {
    let manifest = progress.manifest();
    let manifest_blob = progress
        .manifest_blob()
        .map(CanonicalDigest::to_lowercase_hex);
    let proof_set_id = manifest.map(|manifest| manifest.proof_set_id().to_lowercase_hex());
    let candidate_blob = manifest.map(|manifest| manifest.candidate_blob().to_lowercase_hex());
    let closed_region_blob = manifest
        .and_then(|manifest| manifest.closed_region_blob())
        .map(CanonicalDigest::to_lowercase_hex);
    let mode = manifest.map(|manifest| probe_mode_name(manifest.mode()));

    writer.raw(b"{")?;
    writer.member_string("phase", probe_phase_name(progress.phase()))?;
    writer.raw(b",")?;
    writer.member_bool("prepared", manifest.is_some())?;
    writer.raw(b",")?;
    writer.member_bool(
        "coverage_accepted",
        matches!(
            progress.phase(),
            ExactSourceProbePhaseV1::CoverageAccepted
                | ExactSourceProbePhaseV1::CandidateActive
                | ExactSourceProbePhaseV1::Complete
        ),
    )?;
    writer.raw(b",")?;
    writer.member_bool(
        "candidate_evaluation_active",
        matches!(progress.phase(), ExactSourceProbePhaseV1::CandidateActive),
    )?;
    writer.raw(b",")?;
    writer.member_bool("complete", progress.complete())?;
    writer.raw(b",")?;
    writer.member_optional_string("mode", mode)?;
    writer.raw(b",")?;
    writer.member_optional_string("manifest_blob", manifest_blob.as_deref())?;
    writer.raw(b",")?;
    writer.member_optional_string("proof_set_id", proof_set_id.as_deref())?;
    writer.raw(b",")?;
    writer.member_optional_string("candidate_blob", candidate_blob.as_deref())?;
    writer.raw(b",")?;
    writer.member_optional_string("closed_region_blob", closed_region_blob.as_deref())?;
    writer.raw(b",\"limits\":{")?;
    writer.member_decimal(
        "outer_profile_limit",
        DEFAULT_SOURCE_PROOF_PROFILE_LIMIT.get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "adapter_reachable_site_limit",
        SOURCE_PROOF_ADAPTER_LIMITS_V1.max_reachable_sites.get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "adapter_abstract_step_limit_per_profile",
        SOURCE_PROOF_ADAPTER_LIMITS_V1.max_abstract_steps.get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "adapter_call_depth_limit",
        SOURCE_PROOF_ADAPTER_LIMITS_V1.max_call_depth.get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "adapter_collection_item_limit",
        SOURCE_PROOF_ADAPTER_LIMITS_V1.max_collection_items.get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "adapter_residual_limit",
        SOURCE_PROOF_ADAPTER_LIMITS_V1.max_residuals.get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "extraction_candidate_ordinal_limit_per_profile",
        SOURCE_PROOF_EXTRACTION_OPTIONS_V1
            .max_candidate_ordinals
            .get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "extraction_event_cut_limit_per_profile",
        SOURCE_PROOF_EXTRACTION_OPTIONS_V1.max_event_cuts.get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "classification_refinement_cell_limit_per_profile",
        SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1
            .max_refinement_cells
            .get() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "candidate_rank_limit",
        source_proof_candidate_rank_limit_v1() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "candidate_rank_byte_limit",
        source_proof_candidate_rank_bytes_limit_v1() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "closed_region_limit",
        source_proof_closed_region_limit_v1() as u128,
    )?;
    writer.raw(b"}")?;
    writer.raw(b",\"candidates\":{")?;
    writer.member_decimal(
        "discovered",
        manifest.map_or(0, |manifest| manifest.candidate_count()),
    )?;
    writer.raw(b",")?;
    writer.member_decimal("evaluated", progress.evaluated_candidate_count())?;
    writer.raw(b",")?;
    writer.member_decimal("remaining", progress.remaining_candidate_count())?;
    writer.raw(b"},\"prepared_coverage\":")?;
    let Some(manifest) = manifest else {
        writer.raw(b"null}")?;
        return Ok(());
    };
    let coverage = manifest.coverage();
    writer.raw(b"{")?;
    writer.member_bool(
        "fallback_selected",
        manifest.mode() == ExactSourceProbeModeV1::CanonicalFallback,
    )?;
    writer.raw(b",")?;
    writer.member_decimal("universe_case_count", coverage.universe_case_count())?;
    writer.raw(b",")?;
    writer.member_decimal(
        "certified_closed_case_count",
        coverage.certified_closed_case_count(),
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "residual_open_case_count",
        coverage.residual_open_case_count(),
    )?;
    writer.raw(b",\"source_summary\":")?;
    if coverage.source_summary_available() {
        writer.raw(b"{")?;
        writer.member_decimal(
            "boundary_rank_stride",
            coverage
                .boundary_rank_stride()
                .expect("available source-probe summary has a boundary stride"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "total_outer_profiles",
            coverage
                .total_outer_profiles()
                .expect("available source-probe summary has total profiles"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "analyzed_outer_profiles",
            coverage
                .analyzed_outer_profiles()
                .expect("available source-probe summary has analyzed profiles"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "proof_incomplete_profiles",
            coverage
                .proof_incomplete_profiles()
                .expect("available source-probe summary has incomplete-proof profiles"),
        )?;
        writer.raw(b",")?;
        writer.member_bool(
            "profile_limit_reached",
            coverage
                .profile_limit_reached()
                .expect("available source-probe summary has profile cap status"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "sealed_proof_nonmatch_cases",
            coverage
                .sealed_proof_nonmatch_cases()
                .expect("available source-probe summary has sealed proof nonmatches"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "open_proof_nonmatch_cases",
            coverage
                .open_proof_nonmatch_cases()
                .expect("available source-probe summary has open proof nonmatches"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "open_proof_match_cases",
            coverage
                .open_proof_match_cases()
                .expect("available source-probe summary has open proof matches"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "sealed_structural_excluded_cases",
            coverage
                .sealed_structural_excluded_cases()
                .expect("available source-probe summary has sealed structural exclusions"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "open_structural_excluded_cases",
            coverage
                .open_structural_excluded_cases()
                .expect("available source-probe summary has open structural exclusions"),
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "unaccounted_open_case_count",
            coverage
                .unaccounted_open_case_count()
                .expect("validated source-probe summary has bounded open accounting"),
        )?;
        writer.raw(b",")?;
        writer.member_bool(
            "region_limit_reached",
            coverage
                .region_limit_reached()
                .expect("available source-probe summary has region cap status"),
        )?;
        writer.raw(b",")?;
        writer.member_bool(
            "candidate_limit_reached",
            coverage
                .candidate_limit_reached()
                .expect("available source-probe summary has candidate cap status"),
        )?;
        writer.raw(b"}")?;
    } else {
        writer.raw(b"null")?;
    }
    writer.raw(b"}}")
}

fn probe_phase_name(phase: ExactSourceProbePhaseV1) -> &'static str {
    match phase {
        ExactSourceProbePhaseV1::Unprepared => "unprepared",
        ExactSourceProbePhaseV1::Prepared => "prepared",
        ExactSourceProbePhaseV1::CoverageAccepted => "coverage_accepted",
        ExactSourceProbePhaseV1::CandidateActive => "candidate_active",
        ExactSourceProbePhaseV1::Complete => "complete",
    }
}

fn probe_mode_name(mode: ExactSourceProbeModeV1) -> &'static str {
    match mode {
        ExactSourceProbeModeV1::CheckedSourceProof => "checked_source_proof",
        ExactSourceProbeModeV1::CanonicalFallback => "canonical_fallback",
    }
}

fn write_observable_configuration(
    writer: &mut CanonicalJsonWriter,
    configuration: &ExactObservableConfigurationV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    writer.member_string(
        "program_hash",
        &configuration.program_hash.to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "analysis_program_hash",
        &configuration.analysis_program_hash.to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string("query_hash", &configuration.query_hash.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string("domain_hash", &configuration.domain_hash.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string(
        "report_request_hash",
        &configuration.report_request_hash.to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "probe_plan_hash",
        &configuration.probe_plan_hash.to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "evaluator_contract_hash",
        &configuration.evaluator_contract_hash.to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "mechanism_observation_hash",
        &configuration.mechanism_observation_hash.to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "retention_authorization_hash",
        &configuration
            .retention_authorization_hash
            .to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_decimal("universe_case_count", configuration.universe_case_count)?;
    writer.raw(b",\"bounds\":{")?;
    writer.member_decimal(
        "global_value_node_limit",
        CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2 as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "global_value_semantic_byte_limit",
        CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2 as u128,
    )?;
    writer.raw(b",")?;
    writer.member_bool(
        "fixed_values_all_included",
        configuration
            .fixed_facts
            .iter()
            .all(|fact| fact.value.is_some()),
    )?;
    writer.raw(b",\"dimensions\":[")?;
    for (index, axis) in configuration.axes.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_string("name", &axis.name)?;
        writer.raw(b",")?;
        writer.member_decimal("bound_index", axis.bound_index as u128)?;
        writer.raw(b",")?;
        writer.member_string("role", generator_axis_role_name(axis.role))?;
        writer.raw(b",")?;
        writer.member_decimal("role_field_index", axis.role_field_index as u128)?;
        writer.raw(b",\"domain\":")?;
        write_observable_domain(writer, &axis.domain)?;
        writer.raw(b"}")?;
    }
    writer.raw(b"],\"fixed\":[")?;
    for (index, fact) in configuration.fixed_facts.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_string("name", &fact.name)?;
        writer.raw(b",")?;
        writer.member_decimal("bound_index", fact.bound_index as u128)?;
        writer.raw(b",")?;
        writer.member_string("role", generator_axis_role_name(fact.role))?;
        writer.raw(b",")?;
        writer.member_decimal("role_field_index", fact.role_field_index as u128)?;
        writer.raw(b",\"value\":")?;
        if let Some(value) = &fact.value {
            write_value(writer, value)?;
        } else {
            writer.raw(b"null")?;
        }
        writer.raw(b",")?;
        writer.member_optional_string(
            "omission_reason",
            fact.omission_reason.map(observable_value_omission_name),
        )?;
        writer.raw(b"}")?;
    }
    writer.raw(b"],\"derived\":[")?;
    for (index, fact) in configuration.derived_facts.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_string("name", &fact.name)?;
        writer.raw(b",")?;
        writer.member_decimal("bound_index", fact.bound_index as u128)?;
        writer.raw(b",")?;
        writer.member_string("role", generator_axis_role_name(fact.role))?;
        writer.raw(b",")?;
        writer.member_decimal("role_field_index", fact.role_field_index as u128)?;
        writer.raw(b"}")?;
    }
    writer.raw(b"],")?;
    writer.member_decimal("constraint_count", configuration.constraint_count)?;
    writer.raw(b",")?;
    writer.member_bool("constraint_expressions_included", false)?;
    writer.raw(b"},\"boundary\":")?;
    if let Some(boundary) = &configuration.boundary {
        writer.raw(b"{")?;
        writer.member_string("axis", &boundary.axis)?;
        writer.raw(b",")?;
        writer.member_decimal(
            "axis_dimension_index",
            boundary.axis_dimension_index as u128,
        )?;
        writer.raw(b",")?;
        writer.member_signed_decimal("step", boundary.step)?;
        writer.raw(b",")?;
        writer.member_bool(
            "requires_both_endpoints_in_domain",
            boundary.requires_both_endpoints_in_domain,
        )?;
        writer.raw(b"}")?;
    } else {
        writer.raw(b"null")?;
    }
    writer.raw(b"}")
}

fn write_observable_domain(
    writer: &mut CanonicalJsonWriter,
    domain: &ExactObservableDomainV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    match domain {
        ExactObservableDomainV1::Enumerated {
            cardinality,
            source,
            values,
            omission_reason,
        } => {
            writer.member_string("kind", "values")?;
            writer.raw(b",")?;
            writer.member_decimal("cardinality", *cardinality)?;
            writer.raw(b",\"source\":")?;
            write_enumerated_source(writer, source)?;
            writer.raw(b",")?;
            writer.member_bool("values_included", values.is_some())?;
            writer.raw(b",\"values\":")?;
            if let Some(values) = values {
                writer.raw(b"[")?;
                write_values(writer, values)?;
                writer.raw(b"]")?;
            } else {
                writer.raw(b"null")?;
            }
            writer.raw(b",")?;
            writer.member_optional_string(
                "omission_reason",
                omission_reason.map(observable_value_omission_name),
            )?;
        }
        ExactObservableDomainV1::IntRange {
            start,
            end_exclusive,
            cardinality,
        } => {
            writer.member_string("kind", "int_range")?;
            writer.raw(b",")?;
            writer.member_signed_decimal("start", *start)?;
            writer.raw(b",")?;
            writer.member_signed_decimal("end_exclusive", *end_exclusive)?;
            writer.raw(b",")?;
            writer.member_decimal("cardinality", *cardinality)?;
        }
        ExactObservableDomainV1::FiniteType {
            cardinality,
            inhabitants,
            omission_reason,
        } => {
            writer.member_string("kind", "finite_type")?;
            writer.raw(b",")?;
            writer.member_decimal("cardinality", *cardinality)?;
            writer.raw(b",")?;
            writer.member_bool("inhabitants_included", inhabitants.is_some())?;
            writer.raw(b",\"inhabitants\":")?;
            if let Some(inhabitants) = inhabitants {
                writer.raw(b"[")?;
                write_values(writer, inhabitants)?;
                writer.raw(b"]")?;
            } else {
                writer.raw(b"null")?;
            }
            writer.raw(b",")?;
            writer.member_optional_string(
                "omission_reason",
                omission_reason.map(observable_value_omission_name),
            )?;
        }
    }
    writer.raw(b"}")
}

fn observable_value_omission_name(reason: ExactObservableValueOmissionV1) -> &'static str {
    match reason {
        ExactObservableValueOmissionV1::NodeLimit => "global_value_node_limit",
        ExactObservableValueOmissionV1::SemanticByteLimit => "global_value_semantic_byte_limit",
        ExactObservableValueOmissionV1::NodeAndSemanticByteLimit => {
            "global_value_node_and_semantic_byte_limits"
        }
    }
}

fn write_enumerated_source(
    writer: &mut CanonicalJsonWriter,
    source: &ExactObservableEnumeratedSourceV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    match source {
        ExactObservableEnumeratedSourceV1::ExplicitList => {
            writer.member_string("kind", "explicit_list")?;
        }
        ExactObservableEnumeratedSourceV1::NamedList(name) => {
            writer.member_string("kind", "named_list")?;
            writer.raw(b",")?;
            writer.member_string("name", name)?;
        }
        ExactObservableEnumeratedSourceV1::NamedSet(name) => {
            writer.member_string("kind", "named_set")?;
            writer.raw(b",")?;
            writer.member_string("name", name)?;
        }
    }
    writer.raw(b"}")
}

/// Populations on both sides of the post-aggregation group filter.
///
/// Raw groups continue to partition all projected matches in the reducer. The
/// emitted view is monotone for `varies`: once two different extrema values
/// have been observed, no future evidence can make that group invariant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactGroupAccountingV1 {
    raw_groups: ExactCountBoundV1,
    emitted_groups: ExactCountBoundV1,
    suppressed_groups: ExactCountBoundV1,
    qualifying_configurations: ExactCountBoundV1,
    suppressed_configurations: ExactCountBoundV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExactResultRenderModeV1 {
    ObservablePreview,
    FullPublication,
}

#[derive(Debug, Eq, PartialEq)]
struct ExactPreparedResultsV1 {
    accounting: ExactGroupAccountingV1,
    render_mode: ExactResultRenderModeV1,
    raw_groups_scanned: u128,
    scan_complete: bool,
    rendered_rows: Vec<Vec<u8>>,
    rendered_json_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactSearchDecisionDagTerminalMultiplicityV1 {
    terminal_id: usize,
    terminal: CaseTerminal,
    case_count: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactSearchDecisionDagSummaryV1 {
    terminal_multiplicities: Vec<ExactSearchDecisionDagTerminalMultiplicityV1>,
    excluded: u128,
    eligibility_open: u128,
    admissible_nonmatch: u128,
    admissible_match: u128,
    admissible_open: u128,
}

impl ExactSearchDecisionDagSummaryV1 {
    fn admissibility_closed(&self) -> bool {
        self.eligibility_open == 0
    }

    fn polarity_closed(&self) -> bool {
        self.admissibility_closed() && self.admissible_open == 0
    }

    fn fully_closed_case_count(&self) -> Result<u128, ExactSnapshotRenderError> {
        checked_search_decision_dag_sum(
            checked_search_decision_dag_sum(
                self.excluded,
                self.admissible_nonmatch,
                "fully closed case",
            )?,
            self.admissible_match,
            "fully closed case",
        )
    }

    fn open_case_count(&self) -> Result<u128, ExactSnapshotRenderError> {
        checked_search_decision_dag_sum(self.eligibility_open, self.admissible_open, "open case")
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ExactRenderedSearchDecisionDagPublicationV1 {
    NotRequested,
    Included {
        canonical_graph_object: Vec<u8>,
        artifact_graph_hash: String,
        summary: ExactSearchDecisionDagSummaryV1,
    },
    CapacityLimited {
        resource: ExactSearchDecisionDagPublicationResourceV1,
        maximum: usize,
        required_at_least: usize,
    },
}

impl ExactRenderedSearchDecisionDagPublicationV1 {
    fn requested(&self) -> bool {
        !matches!(self, Self::NotRequested)
    }

    fn complete_for_answer(&self) -> bool {
        match self {
            Self::NotRequested => true,
            Self::Included { summary, .. } => {
                summary.admissibility_closed() && summary.polarity_closed()
            }
            // A deterministic all-or-none capacity result satisfies the view
            // request. Semantic closure is reported independently.
            Self::CapacityLimited { .. } => true,
        }
    }
}

fn prepare_search_decision_dag_publication(
    publication: &ExactPreparedSearchDecisionDagPublicationV1,
    snapshot: &ExactEvidenceSnapshotV1,
    mode: ExactResultRenderModeV1,
) -> Result<ExactRenderedSearchDecisionDagPublicationV1, ExactSnapshotRenderError> {
    match publication {
        ExactPreparedSearchDecisionDagPublicationV1::NotRequested => {
            Ok(ExactRenderedSearchDecisionDagPublicationV1::NotRequested)
        }
        ExactPreparedSearchDecisionDagPublicationV1::CapacityLimited {
            resource,
            maximum,
            required_at_least,
        } => {
            validate_search_decision_dag_capacity_limit(*resource, *maximum, *required_at_least)?;
            Ok(
                ExactRenderedSearchDecisionDagPublicationV1::CapacityLimited {
                    resource: *resource,
                    maximum: *maximum,
                    required_at_least: *required_at_least,
                },
            )
        }
        ExactPreparedSearchDecisionDagPublicationV1::Included(graph) => {
            let summary = validate_search_decision_dag_against_snapshot(graph, snapshot)?;
            if mode == ExactResultRenderModeV1::FullPublication
                && (!summary.admissibility_closed()
                    || !summary.polarity_closed()
                    || snapshot.open_case_count != 0)
            {
                return Err(ExactSnapshotRenderError::invalid(
                    "requested terminal search decision DAG publication must be included with closed admissibility and polarity",
                ));
            }

            let canonical_graph_object = match render_canonical_search_decision_dag_object(graph) {
                Ok(bytes) => bytes,
                Err(error) if error.is_capacity_limit() => {
                    return Ok(
                        ExactRenderedSearchDecisionDagPublicationV1::CapacityLimited {
                            resource:
                                ExactSearchDecisionDagPublicationResourceV1::CanonicalJsonBytes,
                            maximum: EXACT_SEARCH_DECISION_DAG_CANONICAL_JSON_BYTE_LIMIT_V1,
                            required_at_least:
                                EXACT_SEARCH_DECISION_DAG_CANONICAL_JSON_BYTE_LIMIT_V1 + 1,
                        },
                    );
                }
                Err(error) => return Err(error),
            };
            let artifact_graph_hash = exact_stream_blob_sha256(&canonical_graph_object);
            Ok(ExactRenderedSearchDecisionDagPublicationV1::Included {
                canonical_graph_object,
                artifact_graph_hash,
                summary,
            })
        }
    }
}

fn validate_semantic_transition_graph_publication(
    publication: &ExactPreparedSemanticTransitionGraphPublicationV1,
    snapshot: &ExactEvidenceSnapshotV1,
) -> Result<(), ExactSnapshotRenderError> {
    match publication {
        ExactPreparedSemanticTransitionGraphPublicationV1::NotRequested => Ok(()),
        ExactPreparedSemanticTransitionGraphPublicationV1::Included {
            canonical_graph_object,
            artifact_graph_hash,
            state_count: _,
            transition_count,
            constructible_case_count,
        } => {
            if canonical_graph_object.len()
                > EXACT_SEMANTIC_TRANSITION_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1
            {
                return Err(ExactSnapshotRenderError::invalid(
                    "included semantic transition graph exceeds its canonical-JSON limit",
                ));
            }
            if exact_stream_blob_sha256(canonical_graph_object) != *artifact_graph_hash {
                return Err(ExactSnapshotRenderError::invalid(
                    "included semantic transition graph hash disagrees with its canonical bytes",
                ));
            }
            if snapshot.transition_populations.u_c.lower_bound != *constructible_case_count
                || snapshot.transition_populations.u_t.lower_bound != *transition_count
            {
                return Err(ExactSnapshotRenderError::invalid(
                    "included semantic transition graph summary disagrees with transition populations",
                ));
            }
            Ok(())
        }
        ExactPreparedSemanticTransitionGraphPublicationV1::CapacityLimited {
            resource,
            maximum,
            required_at_least,
        } => {
            if *maximum != resource.fixed_maximum() {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "semantic transition graph capacity status for {} names maximum {maximum}, fixed schema maximum is {}",
                    resource.name(),
                    resource.fixed_maximum()
                )));
            }
            if required_at_least <= maximum {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "semantic transition graph capacity status for {} requires at least {required_at_least}, which does not exceed maximum {maximum}",
                    resource.name()
                )));
            }
            Ok(())
        }
    }
}

fn validate_search_decision_dag_capacity_limit(
    resource: ExactSearchDecisionDagPublicationResourceV1,
    maximum: usize,
    required_at_least: usize,
) -> Result<(), ExactSnapshotRenderError> {
    if maximum != resource.fixed_maximum() {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "search decision DAG capacity status for {} names maximum {maximum}, fixed schema maximum is {}",
            resource.name(),
            resource.fixed_maximum()
        )));
    }
    if required_at_least <= maximum {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "search decision DAG capacity status for {} requires at least {required_at_least}, which does not exceed maximum {maximum}",
            resource.name()
        )));
    }
    Ok(())
}

fn validate_search_decision_dag_against_snapshot(
    graph: &CaseDecisionDag,
    snapshot: &ExactEvidenceSnapshotV1,
) -> Result<ExactSearchDecisionDagSummaryV1, ExactSnapshotRenderError> {
    graph.validate().map_err(|error| {
        ExactSnapshotRenderError::invalid(format!(
            "included search decision DAG is invalid: {error}"
        ))
    })?;
    let universe = match graph.universe_cardinality() {
        CheckedCardinality::Exact(universe) => universe,
        CheckedCardinality::ExceedsU128 => {
            return Err(ExactSnapshotRenderError::invalid(
                "included search decision DAG universe exceeds u128::MAX",
            ));
        }
    };
    if universe != snapshot.universe_case_count {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "included search decision DAG universe {universe} disagrees with exact evidence universe {}",
            snapshot.universe_case_count
        )));
    }

    let mut counts = graph.terminal_counts().map_err(|error| {
        ExactSnapshotRenderError::invalid(format!(
            "cannot count included search decision DAG terminals: {error}"
        ))
    })?;
    let mut summary = ExactSearchDecisionDagSummaryV1 {
        terminal_multiplicities: Vec::new(),
        excluded: 0,
        eligibility_open: 0,
        admissible_nonmatch: 0,
        admissible_match: 0,
        admissible_open: 0,
    };
    summary
        .terminal_multiplicities
        .try_reserve_exact(graph.terminals().len())
        .map_err(|_| {
            ExactSnapshotRenderError::limit(
                "cannot allocate included search decision DAG terminal multiplicities",
            )
        })?;
    for (terminal_id, terminal) in graph.terminals().iter().enumerate() {
        let cardinality = counts.remove(terminal).ok_or_else(|| {
            ExactSnapshotRenderError::invalid(format!(
                "included search decision DAG terminal {terminal_id} has no multiplicity"
            ))
        })?;
        let case_count = match cardinality {
            CheckedCardinality::Exact(case_count) => case_count,
            CheckedCardinality::ExceedsU128 => {
                return Err(ExactSnapshotRenderError::invalid(format!(
                    "included search decision DAG terminal {terminal_id} multiplicity exceeds u128::MAX"
                )));
            }
        };
        match terminal {
            CaseTerminal::Excluded => summary.excluded = case_count,
            CaseTerminal::EligibilityOpen(_) => {
                summary.eligibility_open = checked_search_decision_dag_sum(
                    summary.eligibility_open,
                    case_count,
                    "eligibility-open",
                )?;
            }
            CaseTerminal::AdmissibleNonmatch => summary.admissible_nonmatch = case_count,
            CaseTerminal::AdmissibleMatch => summary.admissible_match = case_count,
            CaseTerminal::AdmissibleOpen(_) => {
                summary.admissible_open = checked_search_decision_dag_sum(
                    summary.admissible_open,
                    case_count,
                    "admissible-open",
                )?;
            }
        }
        summary
            .terminal_multiplicities
            .push(ExactSearchDecisionDagTerminalMultiplicityV1 {
                terminal_id,
                terminal: terminal.clone(),
                case_count,
            });
    }
    if !counts.is_empty() {
        return Err(ExactSnapshotRenderError::invalid(
            "included search decision DAG multiplicities contain an unindexed terminal",
        ));
    }

    let fully_closed = summary.fully_closed_case_count()?;
    let open = summary.open_case_count()?;
    if fully_closed != snapshot.closed_case_count || open != snapshot.open_case_count {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "included search decision DAG closed/open multiplicities {fully_closed}/{open} disagree with exact evidence {}/{}",
            snapshot.closed_case_count, snapshot.open_case_count
        )));
    }
    if summary.excluded != snapshot.excluded.lower_bound {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "included search decision DAG excluded multiplicity {} disagrees with exact evidence {}",
            summary.excluded, snapshot.excluded.lower_bound
        )));
    }
    let admissible = checked_search_decision_dag_sum(
        summary.admissible_nonmatch,
        summary.admissible_match,
        "admissible",
    )?;
    if admissible != snapshot.admissible.lower_bound {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "included search decision DAG admissible multiplicity {admissible} disagrees with exact evidence {}",
            snapshot.admissible.lower_bound
        )));
    }
    if summary.admissible_match != snapshot.matching.lower_bound {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "included search decision DAG matching multiplicity {} disagrees with exact evidence {}",
            summary.admissible_match, snapshot.matching.lower_bound
        )));
    }
    Ok(summary)
}

fn checked_search_decision_dag_sum(
    left: u128,
    right: u128,
    label: &str,
) -> Result<u128, ExactSnapshotRenderError> {
    left.checked_add(right).ok_or_else(|| {
        ExactSnapshotRenderError::invalid(format!(
            "included search decision DAG {label} multiplicity exceeds u128::MAX"
        ))
    })
}

fn count_bound(value: u128, exact: bool) -> ExactCountBoundV1 {
    ExactCountBoundV1 {
        lower_bound: value,
        exact: exact.then_some(value),
    }
}

fn group_is_emitted(
    filter: &ExactGroupFilterV1,
    result: &ExactResultAggregateV1,
) -> Result<bool, ExactSnapshotRenderError> {
    match filter {
        ExactGroupFilterV1::All => Ok(true),
        ExactGroupFilterV1::Varies { extrema_index, .. } => result
            .extrema
            .get(*extrema_index)
            .map(|extrema| extrema.minimum < extrema.maximum)
            .ok_or_else(|| {
                ExactSnapshotRenderError::invalid(format!(
                    "varies filter index {extrema_index} has no result extrema aggregate"
                ))
            }),
    }
}

fn group_accounting(
    metadata: &ExactSemanticAnswerMetadataV1,
    snapshot: &ExactEvidenceSnapshotV1,
    scanned_results: &[ExactResultAggregateV1],
    scan_complete: bool,
) -> Result<ExactGroupAccountingV1, ExactSnapshotRenderError> {
    let raw_group_count = snapshot.observed_result_group_count;
    let mut emitted_group_count = 0_u128;
    let mut qualifying_configuration_count = 0_u128;
    let mut scanned_suppressed_configuration_count = 0_u128;
    for result in scanned_results {
        if group_is_emitted(&metadata.group_filter, result)? {
            emitted_group_count = emitted_group_count
                .checked_add(1)
                .ok_or_else(|| ExactSnapshotRenderError::invalid("emitted group count overflow"))?;
            qualifying_configuration_count = qualifying_configuration_count
                .checked_add(result.support.lower_bound)
                .ok_or_else(|| {
                    ExactSnapshotRenderError::invalid("qualifying configuration count overflow")
                })?;
        } else if snapshot.projection_complete {
            scanned_suppressed_configuration_count = scanned_suppressed_configuration_count
                .checked_add(result.support.lower_bound)
                .ok_or_else(|| {
                    ExactSnapshotRenderError::invalid("suppressed configuration count overflow")
                })?;
        }
    }

    if emitted_group_count > scanned_results.len() as u128
        || scanned_results.len() as u128 > raw_group_count
    {
        return Err(ExactSnapshotRenderError::invalid(
            "scanned or emitted group count exceeds observed raw groups",
        ));
    }
    if qualifying_configuration_count > snapshot.matching.lower_bound {
        return Err(ExactSnapshotRenderError::invalid(
            "qualifying configuration count exceeds matching configuration count",
        ));
    }

    let closed = snapshot.projection_complete;
    let accounting = match &metadata.group_filter {
        ExactGroupFilterV1::All => ExactGroupAccountingV1 {
            raw_groups: count_bound(raw_group_count, closed),
            emitted_groups: count_bound(raw_group_count, closed),
            suppressed_groups: count_bound(0, true),
            qualifying_configurations: snapshot.matching,
            suppressed_configurations: count_bound(0, true),
        },
        ExactGroupFilterV1::Varies { .. } if closed && scan_complete => {
            let suppressed_group_count = raw_group_count
                .checked_sub(emitted_group_count)
                .ok_or_else(|| {
                    ExactSnapshotRenderError::invalid("emitted group count exceeds raw group count")
                })?;
            let suppressed_configuration_count = snapshot
                .matching
                .lower_bound
                .checked_sub(qualifying_configuration_count)
                .ok_or_else(|| {
                    ExactSnapshotRenderError::invalid(
                        "qualifying configuration count exceeds matching configuration count",
                    )
                })?;
            ExactGroupAccountingV1 {
                raw_groups: count_bound(raw_group_count, true),
                emitted_groups: count_bound(emitted_group_count, true),
                suppressed_groups: count_bound(suppressed_group_count, true),
                qualifying_configurations: count_bound(qualifying_configuration_count, true),
                suppressed_configurations: count_bound(suppressed_configuration_count, true),
            }
        }
        ExactGroupFilterV1::Varies { .. } if closed => ExactGroupAccountingV1 {
            raw_groups: count_bound(raw_group_count, true),
            emitted_groups: count_bound(emitted_group_count, false),
            suppressed_groups: count_bound(
                (scanned_results.len() as u128)
                    .checked_sub(emitted_group_count)
                    .ok_or_else(|| {
                        ExactSnapshotRenderError::invalid(
                            "emitted group count exceeds scanned raw groups",
                        )
                    })?,
                false,
            ),
            qualifying_configurations: count_bound(qualifying_configuration_count, false),
            suppressed_configurations: count_bound(scanned_suppressed_configuration_count, false),
        },
        ExactGroupFilterV1::Varies { .. } => ExactGroupAccountingV1 {
            raw_groups: count_bound(raw_group_count, false),
            emitted_groups: count_bound(emitted_group_count, false),
            suppressed_groups: count_bound(0, false),
            qualifying_configurations: count_bound(qualifying_configuration_count, false),
            suppressed_configurations: count_bound(0, false),
        },
    };

    Ok(accounting)
}

fn prepare_result_rows(
    metadata: &ExactSemanticAnswerMetadataV1,
    snapshot: &ExactEvidenceSnapshotV1,
    mode: ExactResultRenderModeV1,
) -> Result<ExactPreparedResultsV1, ExactSnapshotRenderError> {
    if mode == ExactResultRenderModeV1::FullPublication && !snapshot.result_group_scan_complete {
        return Err(ExactSnapshotRenderError::limit(
            "full terminal result publication requires an untruncated exact reducer snapshot",
        ));
    }

    let publication_replay_verified = metadata.required_open_obligation_count == 0;
    let mut raw_groups_scanned = 0_usize;
    let mut rendered_rows = Vec::new();
    let mut rendered_json_bytes = 0_usize;
    for result in snapshot.results.iter() {
        if group_is_emitted(&metadata.group_filter, result)? {
            let separator_bytes = if rendered_rows.is_empty() { 0 } else { 1 };
            let row_limit = match mode {
                ExactResultRenderModeV1::ObservablePreview => {
                    let Some(remaining) = EXACT_OBSERVABLE_RESULT_PREVIEW_JSON_BYTE_LIMIT_V1
                        .checked_sub(rendered_json_bytes)
                        .and_then(|remaining| remaining.checked_sub(separator_bytes))
                    else {
                        break;
                    };
                    remaining
                }
                ExactResultRenderModeV1::FullPublication => MAX_TERMINAL_RESULT_ROW_JSON_BYTES_V1
                    .checked_sub(rendered_json_bytes)
                    .and_then(|remaining| remaining.checked_sub(separator_bytes))
                    .ok_or_else(|| {
                        ExactSnapshotRenderError::limit(format!(
                            "terminal result rows exceed the {}-byte atomic publication budget",
                            MAX_TERMINAL_RESULT_ROW_JSON_BYTES_V1
                        ))
                    })?,
            };
            let mut row_writer = CanonicalJsonWriter::with_max_bytes(row_limit);
            if let Err(error) = write_result(
                &mut row_writer,
                &metadata.projection_labels,
                result,
                publication_replay_verified,
            ) {
                if mode == ExactResultRenderModeV1::ObservablePreview {
                    break;
                }
                return Err(error);
            }
            let row = row_writer.finish();
            rendered_json_bytes = rendered_json_bytes
                .checked_add(separator_bytes)
                .and_then(|bytes| bytes.checked_add(row.len()))
                .ok_or_else(|| {
                    ExactSnapshotRenderError::invalid("rendered result-preview byte count overflow")
                })?;
            if mode == ExactResultRenderModeV1::FullPublication
                && rendered_json_bytes > MAX_TERMINAL_RESULT_ROW_JSON_BYTES_V1
            {
                return Err(ExactSnapshotRenderError::limit(format!(
                    "terminal result rows exceed the {}-byte atomic publication budget",
                    MAX_TERMINAL_RESULT_ROW_JSON_BYTES_V1
                )));
            }
            rendered_rows.push(row);
        }
        raw_groups_scanned = raw_groups_scanned
            .checked_add(1)
            .ok_or_else(|| ExactSnapshotRenderError::invalid("scanned raw-group count overflow"))?;
    }

    let scan_complete =
        snapshot.result_group_scan_complete && raw_groups_scanned == snapshot.results.len();
    let scanned_results = &snapshot.results[..raw_groups_scanned];
    let accounting = group_accounting(metadata, snapshot, scanned_results, scan_complete)?;
    Ok(ExactPreparedResultsV1 {
        accounting,
        render_mode: mode,
        raw_groups_scanned: raw_groups_scanned as u128,
        scan_complete,
        rendered_rows,
        rendered_json_bytes,
    })
}

fn render_canonical_semantic_transition_graph_object_v1(
    axis_cardinalities: &[u128],
    schemas: &TransitionSchemaIdentities,
    exact: &ExactEvidenceReducer,
    stream: &ExploreRunStream,
) -> Result<Vec<u8>, ExactSnapshotRenderError> {
    let index = exact.transition_support_index();
    for (schema_id, preimage) in index.iter_state_schemas() {
        if schema_id != schemas.state_schema_id() || preimage != schemas.state_schema_preimage() {
            return Err(ExactSnapshotRenderError::invalid(
                "semantic graph contains a state schema outside the checked transition schema",
            ));
        }
    }
    for (schema_id, preimage) in index.iter_context_schemas() {
        if schema_id != schemas.context_schema_id() || preimage != schemas.context_schema_preimage()
        {
            return Err(ExactSnapshotRenderError::invalid(
                "semantic graph contains a context schema outside the checked transition schema",
            ));
        }
    }
    for (type_id, preimage) in index.iter_transition_types() {
        if type_id != schemas.transition_type_id() || preimage != schemas.transition_type_preimage()
        {
            return Err(ExactSnapshotRenderError::invalid(
                "semantic graph contains a transition type outside the checked transition schema",
            ));
        }
    }

    let mut writer = CanonicalJsonWriter::with_max_bytes(
        EXACT_SEMANTIC_TRANSITION_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1,
    );
    writer.raw(b"{")?;
    writer.member_string("schema", "futuruna.explore.semantic-transition-graph.v1")?;
    writer.raw(b",")?;
    writer.member_u64("schema_version", 1)?;
    writer.raw(b",")?;
    writer.member_string("case_id_encoding", "mixed_radix_rank_intervals_v1")?;
    writer.raw(b",")?;
    writer.member_string("rank_interval_encoding", "half_open")?;
    writer.raw(b",\"axis_cardinalities\":[")?;
    for (index, cardinality) in axis_cardinalities.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.decimal(*cardinality)?;
    }
    writer.raw(b"],")?;
    writer.member_string(
        "state_schema_id",
        &lowercase_hex(&schemas.state_schema_id().bytes()),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "context_schema_id",
        &lowercase_hex(&schemas.context_schema_id().bytes()),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "transition_type_id",
        &lowercase_hex(&schemas.transition_type_id().bytes()),
    )?;
    writer.raw(b",\"states\":[")?;
    for (state_index, (state_id, schema_id, _, value)) in index.iter_states().enumerate() {
        if state_index != 0 {
            writer.raw(b",")?;
        }
        if schema_id != schemas.state_schema_id() {
            return Err(ExactSnapshotRenderError::invalid(
                "semantic graph state has the wrong checked schema identity",
            ));
        }
        writer.raw(b"{")?;
        writer.member_string("state_id", &lowercase_hex(&state_id.bytes()))?;
        writer.raw(b",")?;
        writer.member_string("state_schema_id", &lowercase_hex(&schema_id.bytes()))?;
        writer.raw(b",\"value\":")?;
        write_value(&mut writer, value)?;
        writer.raw(b"}")?;
    }

    let mut rendered_constructible_cases = 0_u128;
    writer.raw(b"],\"transitions\":[")?;
    for (transition_index, (transition_id, transition)) in index.iter().enumerate() {
        if transition_index != 0 {
            writer.raw(b",")?;
        }
        let before_state = index.state(transition.before_state_id()).ok_or_else(|| {
            ExactSnapshotRenderError::invalid(
                "semantic graph transition has no interned before-state",
            )
        })?;
        let after_state = index.state(transition.after_state_id()).ok_or_else(|| {
            ExactSnapshotRenderError::invalid(
                "semantic graph transition has no interned after-state",
            )
        })?;
        if transition.state_schema_id() != schemas.state_schema_id()
            || transition.context_schema_id() != schemas.context_schema_id()
            || transition.transition_type_id() != schemas.transition_type_id()
            || before_state.0 != schemas.state_schema_id()
            || after_state.0 != schemas.state_schema_id()
        {
            return Err(ExactSnapshotRenderError::invalid(
                "semantic graph transition disagrees with its checked schemas or state index",
            ));
        }

        writer.raw(b"{")?;
        writer.member_string("transition_id", &lowercase_hex(&transition_id.bytes()))?;
        writer.raw(b",")?;
        writer.member_string(
            "transition_type_id",
            &lowercase_hex(&transition.transition_type_id().bytes()),
        )?;
        writer.raw(b",")?;
        writer.member_string(
            "before_state_id",
            &lowercase_hex(&transition.before_state_id().bytes()),
        )?;
        writer.raw(b",")?;
        writer.member_string(
            "after_state_id",
            &lowercase_hex(&transition.after_state_id().bytes()),
        )?;
        writer.raw(b",\"context\":")?;
        write_value(&mut writer, transition.context())?;
        writer.raw(b",\"support\":{")?;

        let transition_digest = transition_semantic_fact_digest_from_parts_v2(
            transition_id,
            transition.state_schema_id(),
            transition.context_schema_id(),
            transition.transition_type_id(),
            transition.before_state_id(),
            transition.after_state_id(),
            transition.context(),
            before_state.2,
            after_state.2,
        )
        .map_err(|error| {
            ExactSnapshotRenderError::invalid(format!(
                "cannot derive semantic transition evidence identity: {error}"
            ))
        })?;
        let authenticated_support = stream
            .semantic_transition_support(CanonicalDigest::from_sha256_bytes(
                transition_digest.bytes(),
            ))
            .ok_or_else(|| {
                ExactSnapshotRenderError::invalid(
                    "semantic graph edge has no authenticated transition support",
                )
            })?;

        let mut edge_case_count = 0_u128;
        for (fiber_index, (name, classification)) in [
            (
                "validity_excluded",
                ExactConstructibleClassificationV2::ValidityExcluded,
            ),
            (
                "admissible_nonmatch",
                ExactConstructibleClassificationV2::AdmissibleNonmatch,
            ),
            (
                "admissible_match",
                ExactConstructibleClassificationV2::AdmissibleMatch,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            if fiber_index != 0 {
                writer.raw(b",")?;
            }
            writer.string(name)?;
            writer.raw(b":")?;
            let count = write_semantic_transition_support_v1(
                &mut writer,
                authenticated_support,
                exact.classification_support(classification),
            )?;
            edge_case_count = edge_case_count.checked_add(count).ok_or_else(|| {
                ExactSnapshotRenderError::invalid(
                    "semantic transition edge support count exceeds u128::MAX",
                )
            })?;
        }
        if edge_case_count != authenticated_support.case_count() {
            return Err(ExactSnapshotRenderError::invalid(
                "semantic transition support is not partitioned by exact classifications",
            ));
        }
        rendered_constructible_cases = rendered_constructible_cases
            .checked_add(edge_case_count)
            .ok_or_else(|| {
                ExactSnapshotRenderError::invalid(
                    "semantic transition graph support count exceeds u128::MAX",
                )
            })?;
        writer.raw(b"}}")?;
    }
    if rendered_constructible_cases != exact.transition_population_snapshot().u_c.lower_bound {
        return Err(ExactSnapshotRenderError::invalid(
            "semantic transition edge supports do not partition constructible cases",
        ));
    }
    writer.raw(b"]}")?;
    Ok(writer.finish())
}

fn write_semantic_transition_support_v1(
    writer: &mut CanonicalJsonWriter,
    transition_support: &ExactCaseSupport,
    classification_support: &ExactCaseSupport,
) -> Result<u128, ExactSnapshotRenderError> {
    let intersections = || {
        transition_support
            .intersecting_intervals(classification_support)
            .map_err(|error| {
                ExactSnapshotRenderError::invalid(format!(
                    "cannot intersect transition and classification support: {error}"
                ))
            })
    };
    let case_count = intersections()?.try_fold(0_u128, |count, interval| {
        count.checked_add(interval.case_count()).ok_or_else(|| {
            ExactSnapshotRenderError::invalid("semantic transition support count exceeds u128::MAX")
        })
    })?;
    writer.raw(b"{")?;
    writer.member_decimal("case_count", case_count)?;
    writer.raw(b",\"rank_intervals\":[")?;
    for (index, interval) in intersections()?.enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_decimal("start", interval.start())?;
        writer.raw(b",")?;
        writer.member_decimal("end_exclusive", interval.end_exclusive())?;
        writer.raw(b"}")?;
    }
    writer.raw(b"]}")?;
    Ok(case_count)
}

fn render_canonical_search_decision_dag_object(
    graph: &CaseDecisionDag,
) -> Result<Vec<u8>, ExactSnapshotRenderError> {
    let mut writer =
        CanonicalJsonWriter::with_max_bytes(EXACT_SEARCH_DECISION_DAG_CANONICAL_JSON_BYTE_LIMIT_V1);
    writer.raw(b"{")?;
    writer.member_string("schema", "futuruna.explore.search-decision-dag.v1")?;
    writer.raw(b",")?;
    writer.member_u64("schema_version", 1)?;
    writer.raw(b",")?;
    writer.member_string("ordinal_interval_encoding", "half_open")?;
    writer.raw(b",\"axis_cardinalities\":[")?;
    for (index, cardinality) in graph.axis_cardinalities().iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.decimal(*cardinality)?;
    }
    writer.raw(b"],\"root\":")?;
    write_search_decision_dag_root(&mut writer, graph.root())?;
    writer.raw(b",\"nodes\":[")?;
    for (node_index, node) in graph.nodes().iter().enumerate() {
        if node_index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_decimal("id", node_index)?;
        writer.raw(b",")?;
        writer.member_decimal("dimension_index", node.dimension_index())?;
        writer.raw(b",\"arcs\":[")?;
        for (arc_index, arc) in node.arcs().iter().enumerate() {
            if arc_index != 0 {
                writer.raw(b",")?;
            }
            writer.raw(b"{\"ordinal_intervals\":[")?;
            for (interval_index, interval) in arc.ordinals().intervals().iter().enumerate() {
                if interval_index != 0 {
                    writer.raw(b",")?;
                }
                writer.raw(b"{")?;
                writer.member_decimal("start", interval.start().get())?;
                writer.raw(b",")?;
                writer.member_decimal("end_exclusive", interval.end_exclusive().get())?;
                writer.raw(b"}")?;
            }
            writer.raw(b"],\"target\":")?;
            write_search_decision_dag_ref(&mut writer, arc.child())?;
            writer.raw(b"}")?;
        }
        writer.raw(b"]}")?;
    }
    writer.raw(b"],\"terminals\":[")?;
    for (terminal_index, terminal) in graph.terminals().iter().enumerate() {
        if terminal_index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_decimal("id", terminal_index)?;
        writer.raw(b",")?;
        write_case_terminal_members(&mut writer, terminal)?;
        writer.raw(b"}")?;
    }
    writer.raw(b"]}")?;
    Ok(writer.finish())
}

fn write_search_decision_dag_root(
    writer: &mut CanonicalJsonWriter,
    root: DecisionRoot,
) -> Result<(), ExactSnapshotRenderError> {
    match root {
        DecisionRoot::EmptySpace => writer.raw(b"{\"kind\":\"empty_space\"}"),
        DecisionRoot::Target(target) => write_search_decision_dag_ref(writer, target),
    }
}

fn write_search_decision_dag_ref(
    writer: &mut CanonicalJsonWriter,
    target: DecisionRef,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    match target {
        DecisionRef::Node(id) => {
            writer.member_string("kind", "node")?;
            writer.raw(b",")?;
            writer.member_decimal("id", id.index())?;
        }
        DecisionRef::Terminal(id) => {
            writer.member_string("kind", "terminal")?;
            writer.raw(b",")?;
            writer.member_decimal("id", id.index())?;
        }
    }
    writer.raw(b"}")
}

fn write_case_terminal_members(
    writer: &mut CanonicalJsonWriter,
    terminal: &CaseTerminal,
) -> Result<(), ExactSnapshotRenderError> {
    match terminal {
        CaseTerminal::Excluded => writer.member_string("classification", "excluded"),
        CaseTerminal::EligibilityOpen(reason) => {
            writer.member_string("classification", "eligibility_open")?;
            writer.raw(b",")?;
            writer.member_string("reason", case_open_reason_name(reason))
        }
        CaseTerminal::AdmissibleNonmatch => {
            writer.member_string("classification", "admissible_nonmatch")
        }
        CaseTerminal::AdmissibleMatch => writer.member_string("classification", "admissible_match"),
        CaseTerminal::AdmissibleOpen(reason) => {
            writer.member_string("classification", "admissible_open")?;
            writer.raw(b",")?;
            writer.member_string("reason", case_open_reason_name(reason))
        }
    }
}

fn case_open_reason_name(reason: &CaseOpenReason) -> &'static str {
    match reason {
        CaseOpenReason::SearchBudgetExhausted => "search_budget_exhausted",
        CaseOpenReason::EvaluationUnknown => "evaluation_unknown",
    }
}

fn write_report_request(
    writer: &mut CanonicalJsonWriter,
    search_decision_dag: &ExactRenderedSearchDecisionDagPublicationV1,
    semantic_transition_graph: &ExactPreparedSemanticTransitionGraphPublicationV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    writer.member_string(
        "search_decision_dag",
        if search_decision_dag.requested() {
            "full"
        } else {
            "omit"
        },
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "semantic_transition_graph",
        if semantic_transition_graph.requested() {
            "full"
        } else {
            "omit"
        },
    )?;
    writer.raw(b",")?;
    writer.member_string("matching_ledger", "omit")?;
    writer.raw(b",")?;
    writer.member_string("mechanism_evidence", "unavailable_deferred")?;
    writer.raw(b"}")
}

fn write_graph_envelope(
    writer: &mut CanonicalJsonWriter,
    polarity: ExplorePolarity,
    snapshot: &ExactEvidenceSnapshotV1,
    search_decision_dag: &ExactRenderedSearchDecisionDagPublicationV1,
    semantic_transition_graph: &ExactPreparedSemanticTransitionGraphPublicationV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{\"search_decision_dag\":{")?;
    match search_decision_dag {
        ExactRenderedSearchDecisionDagPublicationV1::NotRequested => {
            writer.member_string("status", "not_requested")?;
            writer.raw(b",\"artifact_graph_hash\":null,\"closure\":null,\"polarity\":null,\"terminal_multiplicities\":null,\"capacity\":null")?;
        }
        ExactRenderedSearchDecisionDagPublicationV1::Included {
            artifact_graph_hash,
            summary,
            ..
        } => {
            writer.member_string("status", "included")?;
            writer.raw(b",")?;
            writer.member_string("artifact_graph_hash", artifact_graph_hash)?;
            writer.raw(b",\"closure\":{")?;
            writer.member_string(
                "admissibility",
                if summary.admissibility_closed() {
                    "closed"
                } else {
                    "open"
                },
            )?;
            writer.raw(b",")?;
            writer.member_string(
                "polarity",
                if summary.polarity_closed() {
                    "closed"
                } else {
                    "open"
                },
            )?;
            writer.raw(b"},")?;
            writer.member_string("polarity", polarity_name(polarity))?;
            writer.raw(b",\"terminal_multiplicities\":[")?;
            for (index, multiplicity) in summary.terminal_multiplicities.iter().enumerate() {
                if index != 0 {
                    writer.raw(b",")?;
                }
                writer.raw(b"{")?;
                writer.member_decimal("terminal_id", multiplicity.terminal_id)?;
                writer.raw(b",\"terminal\":{")?;
                write_case_terminal_members(writer, &multiplicity.terminal)?;
                writer.raw(b"},")?;
                writer.member_decimal("case_count", multiplicity.case_count)?;
                writer.raw(b"}")?;
            }
            writer.raw(b"],\"capacity\":null")?;
        }
        ExactRenderedSearchDecisionDagPublicationV1::CapacityLimited {
            resource,
            maximum,
            required_at_least,
        } => {
            writer.member_string("status", "capacity_limited")?;
            writer.raw(b",\"artifact_graph_hash\":null,\"closure\":{")?;
            writer.member_string(
                "admissibility",
                if snapshot.open_case_count == 0 {
                    "closed"
                } else {
                    "open"
                },
            )?;
            writer.raw(b",")?;
            writer.member_string(
                "polarity",
                if snapshot.open_case_count == 0 {
                    "closed"
                } else {
                    "open"
                },
            )?;
            writer.raw(b"},")?;
            writer.member_string("polarity", polarity_name(polarity))?;
            writer.raw(b",\"terminal_multiplicities\":null,\"capacity\":{")?;
            writer.member_string("resource", resource.name())?;
            writer.raw(b",")?;
            writer.member_decimal("maximum", *maximum)?;
            writer.raw(b",")?;
            writer.member_decimal("required_at_least", *required_at_least)?;
            writer.raw(b"}")?;
        }
    }
    writer.raw(b",\"limits\":")?;
    write_search_decision_dag_limits(writer)?;
    writer.raw(b",\"search_decision_dag\":")?;
    match search_decision_dag {
        ExactRenderedSearchDecisionDagPublicationV1::Included {
            canonical_graph_object,
            ..
        } => writer.raw(canonical_graph_object)?,
        ExactRenderedSearchDecisionDagPublicationV1::NotRequested
        | ExactRenderedSearchDecisionDagPublicationV1::CapacityLimited { .. } => {
            writer.raw(b"null")?
        }
    }
    writer.raw(b"},\"semantic_transition_graph\":{")?;
    write_semantic_transition_graph_envelope_members(
        writer,
        snapshot.transition_populations,
        semantic_transition_graph,
    )?;
    writer.raw(b"},\"mechanism_graph\":{")?;
    writer.member_string("status", "unavailable_deferred")?;
    writer.raw(b"}}")
}

fn write_semantic_transition_graph_envelope_members(
    writer: &mut CanonicalJsonWriter,
    populations: ExactTransitionPopulationSnapshotV2,
    publication: &ExactPreparedSemanticTransitionGraphPublicationV1,
) -> Result<(), ExactSnapshotRenderError> {
    match publication {
        ExactPreparedSemanticTransitionGraphPublicationV1::NotRequested => {
            writer.member_string("status", "not_requested")?;
            writer.raw(b",\"artifact_graph_hash\":null,\"closure\":null,\"state_count\":null,\"transition_count\":null,\"constructible_case_count\":null,\"capacity\":null")?;
        }
        ExactPreparedSemanticTransitionGraphPublicationV1::Included {
            canonical_graph_object: _,
            artifact_graph_hash,
            state_count,
            transition_count,
            constructible_case_count,
        } => {
            writer.member_string("status", "included")?;
            writer.raw(b",")?;
            writer.member_string("artifact_graph_hash", artifact_graph_hash)?;
            writer.raw(b",")?;
            writer.member_string(
                "closure",
                if populations.u_c.exact.is_some() && populations.u_t.exact.is_some() {
                    "closed"
                } else {
                    "open"
                },
            )?;
            writer.raw(b",")?;
            writer.member_decimal("state_count", *state_count)?;
            writer.raw(b",")?;
            writer.member_decimal("transition_count", *transition_count)?;
            writer.raw(b",")?;
            writer.member_decimal("constructible_case_count", *constructible_case_count)?;
            writer.raw(b",\"capacity\":null")?;
        }
        ExactPreparedSemanticTransitionGraphPublicationV1::CapacityLimited {
            resource,
            maximum,
            required_at_least,
        } => {
            writer.member_string("status", "capacity_limited")?;
            writer.raw(b",\"artifact_graph_hash\":null,")?;
            writer.member_string(
                "closure",
                if populations.u_c.exact.is_some() && populations.u_t.exact.is_some() {
                    "closed"
                } else {
                    "open"
                },
            )?;
            writer.raw(b",\"state_count\":null,\"transition_count\":null,\"constructible_case_count\":null,\"capacity\":{")?;
            writer.member_string("resource", resource.name())?;
            writer.raw(b",")?;
            writer.member_decimal("maximum", *maximum)?;
            writer.raw(b",")?;
            writer.member_decimal("required_at_least", *required_at_least)?;
            writer.raw(b"}")?;
        }
    }
    writer.raw(b",\"limits\":{")?;
    writer.member_decimal(
        ExactSemanticTransitionGraphPublicationResourceV1::CanonicalJsonBytes.name(),
        EXACT_SEMANTIC_TRANSITION_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1,
    )?;
    writer.raw(b"},\"semantic_transition_graph\":")?;
    match publication {
        ExactPreparedSemanticTransitionGraphPublicationV1::Included {
            canonical_graph_object,
            ..
        } => writer.raw(canonical_graph_object),
        ExactPreparedSemanticTransitionGraphPublicationV1::NotRequested
        | ExactPreparedSemanticTransitionGraphPublicationV1::CapacityLimited { .. } => {
            writer.raw(b"null")
        }
    }
}

fn write_search_decision_dag_limits(
    writer: &mut CanonicalJsonWriter,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    for (index, resource) in [
        ExactSearchDecisionDagPublicationResourceV1::LoweringAxes,
        ExactSearchDecisionDagPublicationResourceV1::LoweringRankRuns,
        ExactSearchDecisionDagPublicationResourceV1::LoweringNodes,
        ExactSearchDecisionDagPublicationResourceV1::LoweringArcs,
        ExactSearchDecisionDagPublicationResourceV1::LoweringOrdinalIntervals,
        ExactSearchDecisionDagPublicationResourceV1::LoweringAccountedBytes,
        ExactSearchDecisionDagPublicationResourceV1::CanonicalJsonBytes,
    ]
    .into_iter()
    .enumerate()
    {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.member_decimal(resource.name(), resource.fixed_maximum())?;
    }
    writer.raw(b"}")
}

fn write_semantic_answer_object(
    writer: &mut CanonicalJsonWriter,
    metadata: &ExactSemanticAnswerMetadataV1,
    snapshot: &ExactEvidenceSnapshotV1,
    prepared_results: &ExactPreparedResultsV1,
    search_decision_dag_publication: &ExactRenderedSearchDecisionDagPublicationV1,
    semantic_transition_graph_publication: &ExactPreparedSemanticTransitionGraphPublicationV1,
) -> Result<(), ExactSnapshotRenderError> {
    let group_accounting = prepared_results.accounting;
    let classification_closed = snapshot.open_case_count == 0;
    let required_frontier_closed =
        metadata.required_open_case_count == 0 && metadata.required_open_obligation_count == 0;
    let views_complete = search_decision_dag_publication.complete_for_answer()
        && semantic_transition_graph_publication
            .complete_for_answer(snapshot.transition_populations);
    let answer_complete = classification_closed
        && snapshot.projection_complete
        && required_frontier_closed
        && prepared_results.scan_complete
        && views_complete;

    writer.raw(b"{")?;
    writer.member_string("schema", EXACT_SEMANTIC_ANSWER_SCHEMA_V1)?;
    writer.raw(b",")?;
    writer.member_u64("schema_version", SEMANTIC_ANSWER_SCHEMA_VERSION_V1)?;
    writer.raw(b",")?;
    writer.member_string("schema_digest", &metadata.schema_digest.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string(
        "answer_scope_hash",
        &metadata.answer_scope_hash.to_lowercase_hex(),
    )?;
    writer.raw(b",")?;
    writer.member_optional_string("query_name", metadata.query_name.as_deref())?;
    writer.raw(b",")?;
    writer.member_string("polarity", polarity_name(metadata.polarity))?;
    writer.raw(b",")?;
    writer.member_string(
        "status",
        if answer_complete {
            "complete"
        } else {
            "partial"
        },
    )?;
    writer.raw(b",\"closure\":{")?;
    writer.member_string(
        "classification",
        if classification_closed {
            "closed"
        } else {
            "open"
        },
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "projection",
        if snapshot.projection_complete {
            "closed"
        } else {
            "open"
        },
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "required_frontier",
        if required_frontier_closed {
            "closed"
        } else {
            "open"
        },
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "observable_rows",
        if prepared_results.scan_complete {
            "closed"
        } else {
            "open"
        },
    )?;
    writer.raw(b",")?;
    writer.member_string("views", if views_complete { "closed" } else { "open" })?;
    writer.raw(b"},\"group_filter\":")?;
    write_group_filter(writer, &metadata.group_filter)?;
    writer.raw(b",\"case_frontier\":{")?;
    writer.member_decimal("universe_case_count", snapshot.universe_case_count)?;
    writer.raw(b",")?;
    writer.member_decimal("closed_case_count", snapshot.closed_case_count)?;
    writer.raw(b",")?;
    writer.member_decimal("open_case_count", snapshot.open_case_count)?;
    writer.raw(b"},\"required_frontier\":{")?;
    writer.member_decimal("open_case_count", metadata.required_open_case_count)?;
    writer.raw(b",")?;
    writer.member_decimal(
        "open_obligation_count",
        metadata.required_open_obligation_count,
    )?;
    writer.raw(b",")?;
    writer.member_bool("closed", required_frontier_closed)?;
    writer.raw(b"},\"counts\":{")?;
    writer.string("excluded_configurations")?;
    writer.raw(b":")?;
    write_count_bound(writer, snapshot.excluded)?;
    writer.raw(b",")?;
    writer.string("admissible_configurations")?;
    writer.raw(b":")?;
    write_count_bound(writer, snapshot.admissible)?;
    writer.raw(b",")?;
    writer.string("matching_configurations")?;
    writer.raw(b":")?;
    write_count_bound(writer, snapshot.matching)?;
    writer.raw(b",")?;
    writer.string("distinct_result_groups")?;
    writer.raw(b":")?;
    write_count_bound(writer, group_accounting.emitted_groups)?;
    writer.raw(b"},\"transition_populations\":")?;
    write_transition_populations(writer, snapshot.transition_populations)?;
    writer.raw(b",\"group_accounting\":{")?;
    writer.string("raw_groups")?;
    writer.raw(b":")?;
    write_count_bound(writer, group_accounting.raw_groups)?;
    writer.raw(b",")?;
    writer.string("emitted_groups")?;
    writer.raw(b":")?;
    write_count_bound(writer, group_accounting.emitted_groups)?;
    writer.raw(b",")?;
    writer.string("suppressed_groups")?;
    writer.raw(b":")?;
    write_count_bound(writer, group_accounting.suppressed_groups)?;
    writer.raw(b",")?;
    writer.string("qualifying_configurations")?;
    writer.raw(b":")?;
    write_count_bound(writer, group_accounting.qualifying_configurations)?;
    writer.raw(b",")?;
    writer.string("suppressed_configurations")?;
    writer.raw(b":")?;
    write_count_bound(writer, group_accounting.suppressed_configurations)?;
    writer.raw(b"},\"projection\":{")?;
    writer.member_decimal(
        "projected_matching_case_count",
        snapshot.projected_matching_case_count,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "unprojected_matching_case_count",
        snapshot.unprojected_matching_case_count,
    )?;
    writer.raw(b",")?;
    writer.member_bool("complete", snapshot.projection_complete)?;
    writer.raw(b"},\"projection_schema\":{")?;
    writer.string("key")?;
    writer.raw(b":[")?;
    write_strings(writer, &metadata.projection_labels.key)?;
    writer.raw(b"],\"extrema\":[")?;
    write_strings(writer, &metadata.projection_labels.extrema)?;
    writer.raw(b"],\"shown\":[")?;
    write_strings(writer, &metadata.projection_labels.shown)?;
    writer.raw(b"],\"value_encoding\":\"typed_exact_v1\"")?;
    writer.raw(b"},\"report_request\":")?;
    write_report_request(
        writer,
        search_decision_dag_publication,
        semantic_transition_graph_publication,
    )?;
    writer.raw(b",\"graph\":")?;
    write_graph_envelope(
        writer,
        metadata.polarity,
        snapshot,
        search_decision_dag_publication,
        semantic_transition_graph_publication,
    )?;
    writer.raw(b",\"mechanism_evidence\":{")?;
    writer.member_string("status", "unavailable_deferred")?;
    writer.raw(b"},\"results_preview\":{")?;
    writer.member_string(
        "publication_mode",
        match prepared_results.render_mode {
            ExactResultRenderModeV1::ObservablePreview => "bounded_observable_preview",
            ExactResultRenderModeV1::FullPublication => "full_terminal_publication",
        },
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "selection",
        match prepared_results.render_mode {
            ExactResultRenderModeV1::ObservablePreview => "canonical_raw_group_prefix_v1",
            ExactResultRenderModeV1::FullPublication => "all_canonical_raw_groups_v1",
        },
    )?;
    writer.raw(b",")?;
    writer.member_string("row_scope", "eligible_within_scanned_raw_prefix")?;
    writer.raw(b",")?;
    writer.member_decimal("observed_raw_groups", snapshot.observed_result_group_count)?;
    writer.raw(b",")?;
    writer.member_decimal("raw_groups_scanned", prepared_results.raw_groups_scanned)?;
    writer.raw(b",")?;
    writer.member_bool("scan_complete", prepared_results.scan_complete)?;
    writer.raw(b",")?;
    writer.member_decimal(
        "rows_returned",
        prepared_results.rendered_rows.len() as u128,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "omitted_observed_raw_groups",
        snapshot
            .observed_result_group_count
            .checked_sub(prepared_results.raw_groups_scanned)
            .ok_or_else(|| {
                ExactSnapshotRenderError::invalid("scanned raw groups exceed observed raw groups")
            })?,
    )?;
    writer.raw(b",")?;
    writer.member_bool("truncated", !prepared_results.scan_complete)?;
    writer.raw(b",")?;
    writer.member_optional_string(
        "truncation_reason",
        (!prepared_results.scan_complete).then_some("bounded_raw_prefix_or_json_limit"),
    )?;
    writer.raw(b",\"limits\":")?;
    if prepared_results.render_mode == ExactResultRenderModeV1::ObservablePreview {
        writer.raw(b"{")?;
        writer.member_decimal(
            "raw_group_limit",
            EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1 as u128,
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "value_node_limit",
            EXACT_OBSERVABLE_RESULT_PREVIEW_VALUE_NODE_LIMIT_V1 as u128,
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "semantic_byte_limit",
            EXACT_OBSERVABLE_RESULT_PREVIEW_SEMANTIC_BYTE_LIMIT_V1 as u128,
        )?;
        writer.raw(b",")?;
        writer.member_decimal(
            "canonical_json_byte_limit",
            EXACT_OBSERVABLE_RESULT_PREVIEW_JSON_BYTE_LIMIT_V1 as u128,
        )?;
        writer.raw(b"}")?;
    } else {
        writer.raw(b"null")?;
    }
    writer.raw(b",")?;
    writer.member_decimal(
        "canonical_json_bytes_returned",
        prepared_results.rendered_json_bytes as u128,
    )?;
    writer.raw(b"},\"results\":[")?;
    for (index, result) in prepared_results.rendered_rows.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(result)?;
    }
    writer.raw(b"],")?;
    writer.member_bool("matching_ledger_included", false)?;
    writer.raw(b"}")
}

fn write_group_filter(
    writer: &mut CanonicalJsonWriter,
    filter: &ExactGroupFilterV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    match filter {
        ExactGroupFilterV1::All => {
            writer.member_string("kind", "all")?;
        }
        ExactGroupFilterV1::Varies {
            extrema_index,
            extrema_name,
        } => {
            writer.member_string("kind", "varies")?;
            writer.raw(b",")?;
            writer.member_decimal("extrema_index", *extrema_index as u128)?;
            writer.raw(b",")?;
            writer.member_string("extrema_name", extrema_name)?;
        }
    }
    writer.raw(b"}")
}

fn write_count_bound(
    writer: &mut CanonicalJsonWriter,
    bound: ExactCountBoundV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    writer.member_decimal("lower_bound", bound.lower_bound)?;
    writer.raw(b",")?;
    writer.member_optional_decimal("exact", bound.exact)?;
    writer.raw(b",")?;
    writer.member_string(
        "certainty",
        if bound.exact.is_some() {
            "exact"
        } else {
            "lower_bound"
        },
    )?;
    writer.raw(b"}")
}

fn write_transition_populations(
    writer: &mut CanonicalJsonWriter,
    populations: ExactTransitionPopulationSnapshotV2,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    for (index, (name, notation, bound)) in [
        ("declared_cases", "U_D", populations.u_d),
        ("constructible_cases", "U_C", populations.u_c),
        ("distinct_declared_transitions", "U_T", populations.u_t),
        ("admissible_cases", "D_C", populations.d_c),
        ("distinct_admissible_transitions", "D_T", populations.d_t),
        ("matching_cases", "M_C", populations.m_c),
        ("distinct_matching_transitions", "M_T", populations.m_t),
    ]
    .into_iter()
    .enumerate()
    {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.string(name)?;
        writer.raw(b":{")?;
        writer.member_string("notation", notation)?;
        writer.raw(b",")?;
        writer.member_decimal("lower_bound", bound.lower_bound)?;
        writer.raw(b",")?;
        writer.member_optional_decimal("exact", bound.exact)?;
        writer.raw(b",")?;
        writer.member_string(
            "certainty",
            if bound.exact.is_some() {
                "exact"
            } else {
                "lower_bound"
            },
        )?;
        writer.raw(b"}")?;
    }
    writer.raw(b"}")
}

fn write_result(
    writer: &mut CanonicalJsonWriter,
    labels: &ExactProjectionLabelsV1,
    result: &ExactResultAggregateV1,
    publication_replay_verified: bool,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{\"key\":[")?;
    write_named_values(writer, &labels.key, &result.key)?;
    writer.raw(b"],\"support\":")?;
    write_count_bound(writer, result.support)?;
    writer.raw(b",\"extrema\":[")?;
    for (index, extrema) in result.extrema.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        write_extrema(
            writer,
            index,
            &labels.extrema[index],
            extrema,
            publication_replay_verified,
        )?;
    }
    writer.raw(b"],\"representative\":{")?;
    writer.member_bool("selection_closed", result.representative_selection_closed)?;
    writer.raw(b",")?;
    writer.member_bool("replay_verified", publication_replay_verified)?;
    writer.raw(b",\"case_id\":")?;
    write_case_id(writer, &result.representative_case_id)?;
    writer.raw(b",\"shown\":[")?;
    write_named_values(writer, &labels.shown, &result.representative_shown)?;
    writer.raw(b"],")?;
    writer.member_optional_signed_decimal("objective", result.representative_objective)?;
    writer.raw(b"}}")
}

fn write_extrema(
    writer: &mut CanonicalJsonWriter,
    index: usize,
    name: &str,
    extrema: &ExactExtremaAggregateV1,
    publication_replay_verified: bool,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    writer.member_decimal("measure_index", index as u128)?;
    writer.raw(b",")?;
    writer.member_string("name", name)?;
    writer.raw(b",")?;
    writer.member_signed_decimal("minimum", extrema.minimum)?;
    writer.raw(b",")?;
    writer.member_signed_decimal("maximum", extrema.maximum)?;
    writer.raw(b",")?;
    writer.member_decimal("spread", extrema.spread)?;
    writer.raw(b",")?;
    writer.member_decimal("observed_support", extrema.observed_support)?;
    writer.raw(b",")?;
    writer.member_decimal("minimum_tie_support", extrema.minimum_tie_support)?;
    writer.raw(b",")?;
    writer.member_decimal("maximum_tie_support", extrema.maximum_tie_support)?;
    writer.raw(b",\"minimum_witness\":")?;
    write_case_id(writer, &extrema.minimum_witness)?;
    writer.raw(b",\"maximum_witness\":")?;
    write_case_id(writer, &extrema.maximum_witness)?;
    writer.raw(b",")?;
    writer.member_bool("closed", extrema.closed)?;
    writer.raw(b",")?;
    writer.member_bool("witness_replay_verified", publication_replay_verified)?;
    writer.raw(b"}")
}

fn write_case_id(
    writer: &mut CanonicalJsonWriter,
    case_id: &ExactCanonicalCaseIdV1,
) -> Result<(), ExactSnapshotRenderError> {
    writer.raw(b"{")?;
    writer.member_decimal("rank", case_id.rank)?;
    writer.raw(b",\"ordinals\":[")?;
    for (index, ordinal) in case_id.ordinals.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.decimal(*ordinal)?;
    }
    writer.raw(b"]}")
}

fn write_values(
    writer: &mut CanonicalJsonWriter,
    values: &[ExploreValue],
) -> Result<(), ExactSnapshotRenderError> {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        write_value(writer, value)?;
    }
    Ok(())
}

fn write_named_values(
    writer: &mut CanonicalJsonWriter,
    names: &[String],
    values: &[ExploreValue],
) -> Result<(), ExactSnapshotRenderError> {
    for (index, (name, value)) in names.iter().zip(values).enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_string("name", name)?;
        writer.raw(b",\"value\":")?;
        write_value(writer, value)?;
        writer.raw(b"}")?;
    }
    Ok(())
}

fn write_strings(
    writer: &mut CanonicalJsonWriter,
    values: &[String],
) -> Result<(), ExactSnapshotRenderError> {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.string(value)?;
    }
    Ok(())
}

/// Typed JSON avoids conflating Futuruna constructors, tuples, sets and lists,
/// and preserves every float by its exact IEEE-754 bit pattern.
fn write_value(
    writer: &mut CanonicalJsonWriter,
    value: &ExploreValue,
) -> Result<(), ExactSnapshotRenderError> {
    match value {
        ExploreValue::Int(value) => {
            writer.raw(b"{\"kind\":\"int\",")?;
            writer.member_signed_decimal("value", *value)?;
            writer.raw(b"}")
        }
        ExploreValue::FloatBits(bits) => {
            writer.raw(b"{\"kind\":\"float_bits\",")?;
            writer.member_string("bits", &format!("{bits:016x}"))?;
            writer.raw(b"}")
        }
        ExploreValue::String(value) => {
            writer.raw(b"{\"kind\":\"string\",")?;
            writer.member_string("value", value)?;
            writer.raw(b"}")
        }
        ExploreValue::Character(value) => {
            writer.raw(b"{\"kind\":\"character\",")?;
            writer.member_string("value", &value.to_string())?;
            writer.raw(b"}")
        }
        ExploreValue::Boolean(value) => {
            writer.raw(b"{\"kind\":\"boolean\",")?;
            writer.member_bool("value", *value)?;
            writer.raw(b"}")
        }
        ExploreValue::Unit => writer.raw(b"{\"kind\":\"unit\"}"),
        ExploreValue::List(values) => {
            writer.raw(b"{\"kind\":\"list\",\"values\":[")?;
            write_values(writer, values)?;
            writer.raw(b"]}")
        }
        ExploreValue::Set(values) => {
            writer.raw(b"{\"kind\":\"set\",\"values\":[")?;
            write_values(writer, values)?;
            writer.raw(b"]}")
        }
        ExploreValue::Tuple(values) => {
            writer.raw(b"{\"kind\":\"tuple\",\"values\":[")?;
            write_values(writer, values)?;
            writer.raw(b"]}")
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            writer.raw(b"{\"kind\":\"constructor\",")?;
            writer.member_string("type_name", type_name)?;
            writer.raw(b",")?;
            writer.member_string("variant", variant)?;
            writer.raw(b",")?;
            writer.member_bool("positional", *positional)?;
            writer.raw(b",\"fields\":[")?;
            for (index, (name, value)) in fields.iter().enumerate() {
                if index != 0 {
                    writer.raw(b",")?;
                }
                writer.raw(b"{")?;
                writer.member_string("name", name)?;
                writer.raw(b",\"value\":")?;
                write_value(writer, value)?;
                writer.raw(b"}")?;
            }
            writer.raw(b"]}")
        }
    }
}

fn validate_snapshot(
    snapshot: &ExactEvidenceSnapshotV1,
    metadata: &ExactSemanticAnswerMetadataV1,
    result_mode: ExactResultRenderModeV1,
) -> Result<ExactPreparedResultsV1, ExactSnapshotRenderError> {
    metadata.projection_labels.validate()?;
    metadata
        .group_filter
        .validate(&metadata.projection_labels)?;
    let classification_closed = snapshot.open_case_count == 0;
    let recombined_frontier = snapshot
        .closed_case_count
        .checked_add(snapshot.open_case_count)
        .ok_or_else(|| ExactSnapshotRenderError::invalid("case frontier count overflow"))?;
    if recombined_frontier != snapshot.universe_case_count {
        return Err(ExactSnapshotRenderError::invalid(
            "closed and open case counts do not partition the universe",
        ));
    }
    if metadata.required_open_case_count != snapshot.open_case_count {
        return Err(ExactSnapshotRenderError::invalid(
            "required frontier open-case count disagrees with exact evidence snapshot",
        ));
    }

    validate_bound("excluded", snapshot.excluded, classification_closed)?;
    validate_bound("admissible", snapshot.admissible, classification_closed)?;
    validate_bound("matching", snapshot.matching, classification_closed)?;
    validate_transition_populations(
        snapshot.transition_populations,
        snapshot.universe_case_count,
        snapshot.admissible,
        snapshot.matching,
        classification_closed,
    )?;

    let classified = snapshot
        .excluded
        .lower_bound
        .checked_add(snapshot.admissible.lower_bound)
        .ok_or_else(|| ExactSnapshotRenderError::invalid("classification count overflow"))?;
    if classified != snapshot.closed_case_count {
        return Err(ExactSnapshotRenderError::invalid(
            "excluded and admissible lower bounds do not partition closed cases",
        ));
    }
    if snapshot.matching.lower_bound > snapshot.admissible.lower_bound {
        return Err(ExactSnapshotRenderError::invalid(
            "matching lower bound exceeds admissible lower bound",
        ));
    }

    let matching_projection = snapshot
        .projected_matching_case_count
        .checked_add(snapshot.unprojected_matching_case_count)
        .ok_or_else(|| ExactSnapshotRenderError::invalid("matching projection count overflow"))?;
    if matching_projection != snapshot.matching.lower_bound {
        return Err(ExactSnapshotRenderError::invalid(
            "projected and unprojected matching counts do not partition observed matches",
        ));
    }
    let expected_projection_complete =
        classification_closed && snapshot.unprojected_matching_case_count == 0;
    if snapshot.projection_complete != expected_projection_complete {
        return Err(ExactSnapshotRenderError::invalid(
            "projection closure disagrees with classification and unprojected support",
        ));
    }

    let disclosed_result_group_count = snapshot.results.len() as u128;
    if disclosed_result_group_count > snapshot.observed_result_group_count {
        return Err(ExactSnapshotRenderError::invalid(
            "disclosed result groups exceed observed raw result groups",
        ));
    }
    if snapshot.result_group_scan_complete
        != (disclosed_result_group_count == snapshot.observed_result_group_count)
    {
        return Err(ExactSnapshotRenderError::invalid(
            "result-prefix scan closure disagrees with observed and disclosed group counts",
        ));
    }
    if result_mode == ExactResultRenderModeV1::ObservablePreview
        && snapshot.results.len() > EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1
    {
        return Err(ExactSnapshotRenderError::invalid(
            "observable result prefix exceeds its identity-bound raw-group limit",
        ));
    }

    let mut projected_group_support = 0_u128;
    let mut previous_key: Option<&[ExploreValue]> = None;
    let mut extrema_width = None;
    for (index, result) in snapshot.results.iter().enumerate() {
        if previous_key.is_some_and(|previous| previous >= result.key.as_ref()) {
            return Err(ExactSnapshotRenderError::invalid(format!(
                "result group {index} is duplicate or outside canonical key order"
            )));
        }
        previous_key = Some(result.key.as_ref());
        validate_result(snapshot, &metadata.projection_labels, index, result)?;
        if extrema_width
            .replace(result.extrema.len())
            .is_some_and(|expected| expected != result.extrema.len())
        {
            return Err(ExactSnapshotRenderError::invalid(
                "result groups disagree on extrema projection width",
            ));
        }
        projected_group_support = projected_group_support
            .checked_add(result.support.lower_bound)
            .ok_or_else(|| ExactSnapshotRenderError::invalid("result support count overflow"))?;
    }
    if projected_group_support > snapshot.projected_matching_case_count {
        return Err(ExactSnapshotRenderError::invalid(
            "disclosed result-group support exceeds projected matching cases",
        ));
    }
    if snapshot.result_group_scan_complete
        && projected_group_support != snapshot.projected_matching_case_count
    {
        return Err(ExactSnapshotRenderError::invalid(
            "complete result-group support does not partition projected matching cases",
        ));
    }

    let prepared = prepare_result_rows(metadata, snapshot, result_mode)?;
    let accounting = prepared.accounting;
    if snapshot.projection_complete && prepared.scan_complete {
        let raw = accounting.raw_groups.exact.ok_or_else(|| {
            ExactSnapshotRenderError::invalid("closed, fully scanned raw-group count is not exact")
        })?;
        let emitted = accounting.emitted_groups.exact.ok_or_else(|| {
            ExactSnapshotRenderError::invalid(
                "closed, fully scanned emitted-group count is not exact",
            )
        })?;
        let suppressed = accounting.suppressed_groups.exact.ok_or_else(|| {
            ExactSnapshotRenderError::invalid(
                "closed, fully scanned suppressed-group count is not exact",
            )
        })?;
        if emitted.checked_add(suppressed) != Some(raw) {
            return Err(ExactSnapshotRenderError::invalid(
                "closed group accounting violates raw = emitted + suppressed",
            ));
        }

        let matching = snapshot.matching.exact.ok_or_else(|| {
            ExactSnapshotRenderError::invalid("closed matching count is not exact")
        })?;
        let qualifying = accounting.qualifying_configurations.exact.ok_or_else(|| {
            ExactSnapshotRenderError::invalid("closed qualifying-configuration count is not exact")
        })?;
        let suppressed_configurations =
            accounting.suppressed_configurations.exact.ok_or_else(|| {
                ExactSnapshotRenderError::invalid(
                    "closed suppressed-configuration count is not exact",
                )
            })?;
        if qualifying.checked_add(suppressed_configurations) != Some(matching) {
            return Err(ExactSnapshotRenderError::invalid(
                "closed group accounting violates qualifying + suppressed configurations = matching configurations",
            ));
        }
    }

    Ok(prepared)
}

fn validate_transition_populations(
    populations: ExactTransitionPopulationSnapshotV2,
    universe_case_count: u128,
    admissible: ExactCountBoundV1,
    matching: ExactCountBoundV1,
    classification_closed: bool,
) -> Result<(), ExactSnapshotRenderError> {
    validate_bound("U_D declared cases", populations.u_d, true)?;
    for (name, bound) in [
        ("U_C constructible cases", populations.u_c),
        ("U_T distinct declared transitions", populations.u_t),
        ("D_C admissible cases", populations.d_c),
        ("D_T distinct admissible transitions", populations.d_t),
        ("M_C matching cases", populations.m_c),
        ("M_T distinct matching transitions", populations.m_t),
    ] {
        validate_bound(name, bound, classification_closed)?;
    }
    if populations.u_d.lower_bound != universe_case_count
        || populations.d_c != admissible
        || populations.m_c != matching
    {
        return Err(ExactSnapshotRenderError::invalid(
            "transition case populations disagree with the exact evidence snapshot",
        ));
    }
    for (subset_name, subset, superset_name, superset) in [
        ("U_C", populations.u_c, "U_D", populations.u_d),
        ("U_T", populations.u_t, "U_C", populations.u_c),
        ("D_C", populations.d_c, "U_C", populations.u_c),
        ("D_T", populations.d_t, "U_T", populations.u_t),
        ("D_T", populations.d_t, "D_C", populations.d_c),
        ("M_C", populations.m_c, "D_C", populations.d_c),
        ("M_T", populations.m_t, "D_T", populations.d_t),
        ("M_T", populations.m_t, "M_C", populations.m_c),
    ] {
        if subset.lower_bound > superset.lower_bound {
            return Err(ExactSnapshotRenderError::invalid(format!(
                "transition population {subset_name} exceeds {superset_name}"
            )));
        }
    }
    Ok(())
}

fn validate_bound(
    name: &str,
    bound: ExactCountBoundV1,
    closed: bool,
) -> Result<(), ExactSnapshotRenderError> {
    match (closed, bound.exact) {
        (true, Some(exact)) if exact == bound.lower_bound => Ok(()),
        (false, None) => Ok(()),
        (true, _) => Err(ExactSnapshotRenderError::invalid(format!(
            "closed {name} count must expose its lower bound as exact"
        ))),
        (false, Some(_)) => Err(ExactSnapshotRenderError::invalid(format!(
            "open {name} count must not claim an exact value"
        ))),
    }
}

fn validate_result(
    snapshot: &ExactEvidenceSnapshotV1,
    labels: &ExactProjectionLabelsV1,
    index: usize,
    result: &ExactResultAggregateV1,
) -> Result<(), ExactSnapshotRenderError> {
    if result.key.len() != labels.key.len()
        || result.extrema.len() != labels.extrema.len()
        || result.representative_shown.len() != labels.shown.len()
    {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "result group {index} projection widths disagree with checked key/extrema/shown labels"
        )));
    }
    if result.support.lower_bound == 0 {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "result group {index} has zero support"
        )));
    }
    validate_bound(
        "result-group support",
        result.support,
        snapshot.projection_complete,
    )?;
    if result.representative_selection_closed != snapshot.projection_complete {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "result group {index} representative closure disagrees with projection closure"
        )));
    }
    validate_case_id(
        "representative",
        snapshot.universe_case_count,
        &result.representative_case_id,
    )?;
    for (measure_index, extrema) in result.extrema.iter().enumerate() {
        validate_extrema(snapshot, index, measure_index, result.support, extrema)?;
    }
    Ok(())
}

fn validate_extrema(
    snapshot: &ExactEvidenceSnapshotV1,
    group_index: usize,
    measure_index: usize,
    support: ExactCountBoundV1,
    extrema: &ExactExtremaAggregateV1,
) -> Result<(), ExactSnapshotRenderError> {
    let label = format!("result group {group_index} extrema {measure_index}");
    if extrema.minimum > extrema.maximum {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "{label} minimum exceeds maximum"
        )));
    }
    let expected_spread = (extrema.maximum as i128 - extrema.minimum as i128) as u128;
    if extrema.spread != expected_spread {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "{label} spread does not equal maximum minus minimum"
        )));
    }
    if extrema.observed_support != support.lower_bound
        || extrema.minimum_tie_support == 0
        || extrema.maximum_tie_support == 0
        || extrema.minimum_tie_support > extrema.observed_support
        || extrema.maximum_tie_support > extrema.observed_support
    {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "{label} support or tie support is inconsistent"
        )));
    }
    if extrema.closed != snapshot.projection_complete {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "{label} closure disagrees with projection closure"
        )));
    }
    validate_case_id(
        "minimum witness",
        snapshot.universe_case_count,
        &extrema.minimum_witness,
    )?;
    validate_case_id(
        "maximum witness",
        snapshot.universe_case_count,
        &extrema.maximum_witness,
    )
}

fn validate_case_id(
    name: &str,
    universe_case_count: u128,
    case_id: &ExactCanonicalCaseIdV1,
) -> Result<(), ExactSnapshotRenderError> {
    if case_id.rank >= universe_case_count {
        return Err(ExactSnapshotRenderError::invalid(format!(
            "{name} rank {} lies outside universe cardinality {universe_case_count}",
            case_id.rank
        )));
    }
    Ok(())
}

fn lifecycle_name(lifecycle: RunLifecycle) -> &'static str {
    match lifecycle {
        RunLifecycle::Running => "running",
        RunLifecycle::Paused => "paused",
        RunLifecycle::Sealed => "sealed",
    }
}

fn polarity_name(polarity: ExplorePolarity) -> &'static str {
    match polarity {
        ExplorePolarity::Matches => "matches",
        ExplorePolarity::Violations => "violations",
    }
}

fn observable_phase_name(phase: ExactObservablePhaseV1) -> &'static str {
    match phase {
        ExactObservablePhaseV1::Probes => "probes",
        ExactObservablePhaseV1::CaseSearch => "case_search",
        ExactObservablePhaseV1::Finalization => "finalization",
        ExactObservablePhaseV1::Complete => "complete",
    }
}

struct CanonicalJsonWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl CanonicalJsonWriter {
    fn new() -> Self {
        Self::with_max_bytes(MAX_CANONICAL_JSON_BYTES)
    }

    fn with_max_bytes(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), ExactSnapshotRenderError> {
        let next_len = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or_else(|| ExactSnapshotRenderError::limit("canonical JSON size overflow"))?;
        if next_len > self.max_bytes {
            return Err(ExactSnapshotRenderError::limit(format!(
                "canonical JSON exceeds {} bytes",
                self.max_bytes
            )));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), ExactSnapshotRenderError> {
        self.raw(b"\"")?;
        let mut start = 0;
        for (index, byte) in value.bytes().enumerate() {
            let escape: Option<&[u8]> = match byte {
                b'\"' => Some(b"\\\""),
                b'\\' => Some(b"\\\\"),
                b'\x08' => Some(b"\\b"),
                b'\x0c' => Some(b"\\f"),
                b'\n' => Some(b"\\n"),
                b'\r' => Some(b"\\r"),
                b'\t' => Some(b"\\t"),
                0x00..=0x1f => None,
                _ => continue,
            };
            self.raw(&value.as_bytes()[start..index])?;
            if let Some(escape) = escape {
                self.raw(escape)?;
            } else {
                let encoded = format!("\\u00{byte:02x}");
                self.raw(encoded.as_bytes())?;
            }
            start = index + 1;
        }
        self.raw(&value.as_bytes()[start..])?;
        self.raw(b"\"")
    }

    fn decimal(&mut self, value: impl fmt::Display) -> Result<(), ExactSnapshotRenderError> {
        self.string(&value.to_string())
    }

    fn member_string(&mut self, name: &str, value: &str) -> Result<(), ExactSnapshotRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.string(value)
    }

    fn member_optional_string(
        &mut self,
        name: &str,
        value: Option<&str>,
    ) -> Result<(), ExactSnapshotRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        if let Some(value) = value {
            self.string(value)
        } else {
            self.raw(b"null")
        }
    }

    fn member_decimal(
        &mut self,
        name: &str,
        value: impl fmt::Display,
    ) -> Result<(), ExactSnapshotRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.decimal(value)
    }

    fn member_signed_decimal(
        &mut self,
        name: &str,
        value: i64,
    ) -> Result<(), ExactSnapshotRenderError> {
        self.member_decimal(name, value)
    }

    fn member_optional_decimal<T: fmt::Display>(
        &mut self,
        name: &str,
        value: Option<T>,
    ) -> Result<(), ExactSnapshotRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        if let Some(value) = value {
            self.decimal(value)
        } else {
            self.raw(b"null")
        }
    }

    fn member_optional_signed_decimal(
        &mut self,
        name: &str,
        value: Option<i64>,
    ) -> Result<(), ExactSnapshotRenderError> {
        self.member_optional_decimal(name, value)
    }

    fn member_bool(&mut self, name: &str, value: bool) -> Result<(), ExactSnapshotRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.raw(if value { b"true" } else { b"false" })
    }

    fn member_u64(&mut self, name: &str, value: u64) -> Result<(), ExactSnapshotRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.raw(value.to_string().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_json_string_encoded_len, ExactPresentationStringBudgetV1,
        MAX_PRESENTATION_STRING_OCCURRENCES_V1, MAX_PROJECTION_LABEL_BYTES,
    };

    #[test]
    fn presentation_budget_uses_exact_canonical_json_string_bytes() {
        assert_eq!(canonical_json_string_encoded_len("plain"), Some(7));
        assert_eq!(
            canonical_json_string_encoded_len("\"\\\n\u{0001}"),
            Some(14)
        );
        assert_eq!(canonical_json_string_encoded_len("ø"), Some(4));

        let maximal_string = "x".repeat(MAX_PROJECTION_LABEL_BYTES);
        let mut budget = ExactPresentationStringBudgetV1::new();
        for _ in 0..7 {
            budget
                .charge("test presentation string", &maximal_string)
                .expect("seven individually maximal strings fit the cumulative JSON budget");
        }
        let error = budget
            .charge("test presentation string", &maximal_string)
            .expect_err("the eighth encoded string crosses the cumulative JSON budget");
        assert!(error.is_capacity_limit());

        let mut occurrence_budget = ExactPresentationStringBudgetV1 {
            encoded_bytes: 0,
            occurrences: MAX_PRESENTATION_STRING_OCCURRENCES_V1,
        };
        let error = occurrence_budget
            .charge("test presentation string", "")
            .expect_err("one occurrence beyond the retained metadata cap is rejected");
        assert!(error.is_capacity_limit());
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
