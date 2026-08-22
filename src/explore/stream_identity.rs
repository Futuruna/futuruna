//! Producer-owned identity for one durable exact Explore stream.
//!
//! This module is deliberately smaller than the execution coordinator.  It
//! consumes only a validated checked-query view and immutable report/evaluator
//! contracts, then binds the complete case universe before any probe, proof or
//! scheduling decision is allowed to run.

use sha2::{Digest, Sha256};

use super::case_graph::{
    DEFAULT_MAX_CASE_RANK_RUNS, DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES,
    DEFAULT_MAX_CASE_RANK_RUN_ARCS, DEFAULT_MAX_CASE_RANK_RUN_AXES,
    DEFAULT_MAX_CASE_RANK_RUN_NODES, DEFAULT_MAX_CASE_RANK_RUN_ORDINAL_INTERVALS,
};
use super::classification_regions::SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1;
use super::exact_stream::{
    EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_JSON_BYTE_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_SEMANTIC_BYTE_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_VALUE_NODE_LIMIT_V1,
};
use super::mechanism::{CheckedMechanismObservationRequestV1, MechanismQueryId};
use super::mechanism_snapshot::mechanism_observable_checkpoint_contract_digest_v1;
use super::mechanism_stream::{
    mechanism_stream_contract_digest_v1, validate_mechanism_stream_request_v1,
};
use super::report::{
    ExploreCaseGraphRequest, ExploreLedgerRequest, ExploreReportRequest,
    DEFAULT_EXPLORE_COLLECTION_LIMIT, DEFAULT_EXPLORE_STEP_LIMIT,
};
use super::run_stream::{
    CanonicalDigest, ExploreCaseUniverse, ExploreRunHeader, ExploreRunIdentity, ExploreRunNonce,
    ExploreRunSchemas, RequiredObligationId,
};
use super::source_events::{SOURCE_PROOF_ADAPTER_LIMITS_V1, SOURCE_PROOF_EXTRACTION_OPTIONS_V1};
use super::source_proof_plan::DEFAULT_SOURCE_PROOF_PROFILE_LIMIT;
use super::stream_probe::SOURCE_PROBE_MANIFEST_MAX_BYTES_V1;
use super::stream_proof::{
    source_proof_candidate_rank_bytes_limit_v1, source_proof_candidate_rank_limit_v1,
    source_proof_closed_region_limit_v1,
};
use super::stream_snapshot::{
    validate_exact_snapshot_presentation_v1, ExactProjectionLabelsV1,
    EXACT_CASE_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1,
    EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_JSON_BYTE_LIMIT_V1, MAX_CANONICAL_JSON_BYTES,
    MAX_PRESENTATION_STRING_JSON_BYTES_V1, MAX_PRESENTATION_STRING_OCCURRENCES_V1,
    MAX_PROJECTION_LABELS, MAX_PROJECTION_LABEL_BYTES, MAX_PROJECTION_LABEL_TOTAL_BYTES_V1,
    MAX_TERMINAL_RESULT_ROW_JSON_BYTES_V1,
};
use crate::TypeCheckArtifacts;

const DIGEST_DOMAIN_V1: &[u8] = b"futuruna.explore.stream-contract-digest.v1";
pub(super) const CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2: usize = 4_096;
pub(super) const CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2: usize = 4 * 1024 * 1024;

/// One report-wide replay closure is known before the result groups exist.
/// Once case classification is closed, the coordinator either fresh-replays
/// every selected representative/extrema witness or proves the set empty,
/// then closes this obligation in one atomic evidence transition.
const REPLAY_CLOSURE_CONTRACT_V1: &[u8] = b"futuruna.explore.required-obligation.replay-closure.v1";

#[derive(Debug, Clone)]
pub(super) struct PreparedExactStreamHeader {
    pub(super) header: ExploreRunHeader,
    pub(super) replay_closure: RequiredObligationId,
}

/// Build sequence-zero identity from the producer-minted checked program and
/// query artifacts.  Operational choices such as run path, shard width, time
/// cap, jobs and resource samples are intentionally absent.
pub(super) fn prepare_exact_stream_header(
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    nonce: CanonicalDigest,
    report_request: ExploreReportRequest,
) -> Result<PreparedExactStreamHeader, String> {
    prepare_exact_stream_header_with_mechanism(
        artifacts,
        accepted_query_index,
        nonce,
        report_request,
        None,
    )
}

/// Private sequence-zero constructor for a stream whose immutable answer
/// contract includes dynamic mechanism evidence. The public CLI continues to
/// call [`prepare_exact_stream_header`], preserving the deferred identity.
pub(super) fn prepare_exact_stream_header_with_mechanism(
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    nonce: CanonicalDigest,
    report_request: ExploreReportRequest,
    mechanism_request: Option<&CheckedMechanismObservationRequestV1>,
) -> Result<PreparedExactStreamHeader, String> {
    if report_request.ledger != ExploreLedgerRequest::Omit {
        return Err(
            "durable exact streams do not yet implement matching-ledger publication".to_string(),
        );
    }
    let checked = artifacts
        .checked_exploration_query(accepted_query_index)
        .map_err(|error| format!("cannot bind checked Explore stream identity: {error:?}"))?;
    validate_exact_snapshot_presentation_v1(checked.closed_query)
        .map_err(|error| format!("cannot bind bounded Explore presentation metadata: {error}"))?;
    ExactProjectionLabelsV1::from_checked_query(checked.closed_query)
        .map_err(|error| format!("cannot bind bounded Explore projection labels: {error}"))?;
    if let Some(request) = mechanism_request {
        validate_mechanism_stream_request_v1(request)
            .map_err(|error| format!("cannot bind checked mechanism stream request: {error}"))?;
        if request.observation.analysis_program.as_str()
            != checked.artifact.identity.analysis_program.as_str()
        {
            return Err(
                "checked mechanism request belongs to another analysis program".to_string(),
            );
        }
        let expected_query = MechanismQueryId::from_checked_query(&checked)
            .map_err(|error| format!("cannot bind checked mechanism query identity: {error}"))?;
        if request.observation.query != expected_query {
            return Err(
                "checked mechanism request belongs to another Explore query or domain".to_string(),
            );
        }
    }

    let axis_cardinalities = checked
        .closed_query
        .universe
        .dimensions
        .iter()
        .map(|dimension| {
            dimension.domain.cardinality().exact().ok_or_else(|| {
                format!(
                    "Explore dimension `{}` cardinality exceeds u128::MAX",
                    dimension.name
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(request) = mechanism_request {
        if request.observation.axis_cardinalities.as_ref() != axis_cardinalities.as_slice() {
            return Err(
                "checked mechanism request belongs to another Explore case universe".to_string(),
            );
        }
    }
    let case_universe = ExploreCaseUniverse::new(axis_cardinalities.into_boxed_slice())
        .map_err(|error| error.to_string())?;

    let schemas = ExploreRunSchemas::new(
        contract_digest(&[
            b"journal-record",
            b"canonical-binary-v3",
            b"append-only-source-probe-tags-v1",
            b"prepared-manifest-codec-v1",
            b"prepared-manifest-byte-limit",
            &usize_bytes(SOURCE_PROBE_MANIFEST_MAX_BYTES_V1),
            b"snapshot-unavailable-discovery-v1",
        ]),
        contract_digest(&[
            b"semantic-evidence",
            b"normalized-merkle-treap-v2",
            b"persistent-case-support-treap-v2",
        ]),
        if mechanism_request.is_some() {
            contract_digest(&[
                b"mechanism-snapshot",
                b"mechanism-observable-checkpoint-v1",
                &mechanism_stream_contract_digest_v1(),
                &mechanism_observable_checkpoint_contract_digest_v1(),
            ])
        } else {
            contract_digest(&[
                b"exact-snapshot",
                b"exact-observable-snapshot-v6",
                b"grouped-having-filter-v1",
                b"bounded-canonical-raw-group-preview-v1",
                b"result-preview-group-limit",
                &usize_bytes(EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1),
                b"result-preview-value-node-limit",
                &usize_bytes(EXACT_OBSERVABLE_RESULT_PREVIEW_VALUE_NODE_LIMIT_V1),
                b"result-preview-semantic-byte-limit",
                &usize_bytes(EXACT_OBSERVABLE_RESULT_PREVIEW_SEMANTIC_BYTE_LIMIT_V1),
                b"result-preview-json-byte-limit",
                &usize_bytes(EXACT_OBSERVABLE_RESULT_PREVIEW_JSON_BYTE_LIMIT_V1),
                b"observable-snapshot-outer-canonical-json-byte-limit-v1",
                &usize_bytes(MAX_CANONICAL_JSON_BYTES),
                b"projection-label-count-limit",
                &usize_bytes(MAX_PROJECTION_LABELS),
                b"projection-label-byte-limit",
                &usize_bytes(MAX_PROJECTION_LABEL_BYTES),
                b"projection-label-total-byte-limit",
                &usize_bytes(MAX_PROJECTION_LABEL_TOTAL_BYTES_V1),
                b"checked-presentation-string-budget-v1",
                b"presentation-string-canonical-json-byte-limit",
                &usize_bytes(MAX_PRESENTATION_STRING_JSON_BYTES_V1),
                b"presentation-string-occurrence-limit",
                &usize_bytes(MAX_PRESENTATION_STRING_OCCURRENCES_V1),
                b"cursor-bound-snapshot-unavailable-v1",
                b"snapshot-unavailable-json-byte-limit",
                &usize_bytes(EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_JSON_BYTE_LIMIT_V1),
                b"inspectable-configuration-manifest-v3",
                b"configuration-value-node-limit",
                &usize_bytes(CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2),
                b"configuration-value-semantic-byte-limit",
                &usize_bytes(CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2),
                b"replay-derived-source-probe-progress-v1",
                b"inspectable-source-probe-limits-v1",
                b"explicit-case-graph-publication-v1",
                b"mixed-radix-rank-run-lowerer-v1",
                b"case-graph-axis-limit",
                &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_AXES),
                b"case-graph-rank-run-limit",
                &usize_bytes(DEFAULT_MAX_CASE_RANK_RUNS),
                b"case-graph-node-limit",
                &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_NODES),
                b"case-graph-arc-limit",
                &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_ARCS),
                b"case-graph-ordinal-interval-limit",
                &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_ORDINAL_INTERVALS),
                b"case-graph-accounted-byte-limit",
                &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES),
                b"case-graph-canonical-json-byte-limit",
                &usize_bytes(EXACT_CASE_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1),
            ])
        },
        contract_digest(&[
            b"terminal-result",
            b"exact-report-v5",
            b"grouped-having-filter-v1",
            b"full-result-publication-required-v1",
            b"terminal-result-row-json-byte-limit-v1",
            &usize_bytes(MAX_TERMINAL_RESULT_ROW_JSON_BYTES_V1),
            b"terminal-outer-canonical-json-byte-limit-v1",
            &usize_bytes(MAX_CANONICAL_JSON_BYTES),
            b"projection-label-count-limit",
            &usize_bytes(MAX_PROJECTION_LABELS),
            b"projection-label-byte-limit",
            &usize_bytes(MAX_PROJECTION_LABEL_BYTES),
            b"projection-label-total-byte-limit",
            &usize_bytes(MAX_PROJECTION_LABEL_TOTAL_BYTES_V1),
            b"checked-presentation-string-budget-v1",
            b"presentation-string-canonical-json-byte-limit",
            &usize_bytes(MAX_PRESENTATION_STRING_JSON_BYTES_V1),
            b"presentation-string-occurrence-limit",
            &usize_bytes(MAX_PRESENTATION_STRING_OCCURRENCES_V1),
            b"explicit-case-graph-publication-v1",
            b"mixed-radix-rank-run-lowerer-v1",
            b"case-graph-axis-limit",
            &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_AXES),
            b"case-graph-rank-run-limit",
            &usize_bytes(DEFAULT_MAX_CASE_RANK_RUNS),
            b"case-graph-node-limit",
            &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_NODES),
            b"case-graph-arc-limit",
            &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_ARCS),
            b"case-graph-ordinal-interval-limit",
            &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_ORDINAL_INTERVALS),
            b"case-graph-accounted-byte-limit",
            &usize_bytes(DEFAULT_MAX_CASE_RANK_RUN_ACCOUNTED_BYTES),
            b"case-graph-canonical-json-byte-limit",
            &usize_bytes(EXACT_CASE_GRAPH_CANONICAL_JSON_BYTE_LIMIT_V1),
        ]),
    );
    let identity = ExploreRunIdentity::new(
        parse_checked_digest("program_hash", checked.program_hash())?,
        parse_checked_digest(
            "analysis_program_hash",
            checked.artifact.identity.analysis_program.as_str(),
        )?,
        parse_checked_digest("query_hash", &checked.artifact.identity.digest)?,
        parse_checked_digest("domain_hash", checked.domain_hash())?,
        report_request_digest(report_request),
        contract_digest(&[
            b"probe-plan",
            b"source-proof-prepared-manifest-phase-v2",
            b"candidate-evaluation-before-residual-v1",
            b"outer-profile-limit",
            &usize_bytes(DEFAULT_SOURCE_PROOF_PROFILE_LIMIT.get()),
            b"adapter-reachable-site-limit",
            &usize_bytes(SOURCE_PROOF_ADAPTER_LIMITS_V1.max_reachable_sites.get()),
            b"adapter-abstract-step-limit-per-profile",
            &usize_bytes(SOURCE_PROOF_ADAPTER_LIMITS_V1.max_abstract_steps.get()),
            b"adapter-call-depth-limit",
            &usize_bytes(SOURCE_PROOF_ADAPTER_LIMITS_V1.max_call_depth.get()),
            b"adapter-collection-item-limit",
            &usize_bytes(SOURCE_PROOF_ADAPTER_LIMITS_V1.max_collection_items.get()),
            b"adapter-residual-limit",
            &usize_bytes(SOURCE_PROOF_ADAPTER_LIMITS_V1.max_residuals.get()),
            b"extraction-candidate-ordinal-limit-per-profile",
            &usize_bytes(
                SOURCE_PROOF_EXTRACTION_OPTIONS_V1
                    .max_candidate_ordinals
                    .get(),
            ),
            b"extraction-event-cut-limit-per-profile",
            &usize_bytes(SOURCE_PROOF_EXTRACTION_OPTIONS_V1.max_event_cuts.get()),
            b"classification-refinement-cell-limit-per-profile",
            &usize_bytes(
                SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1
                    .max_refinement_cells
                    .get(),
            ),
            b"candidate-rank-limit",
            &usize_bytes(source_proof_candidate_rank_limit_v1()),
            b"candidate-rank-byte-limit",
            &usize_bytes(source_proof_candidate_rank_bytes_limit_v1()),
            b"closed-region-limit",
            &usize_bytes(source_proof_closed_region_limit_v1()),
        ]),
        contract_digest(&[
            b"evaluator",
            b"exact-transition-interpreter-v2",
            b"normalized-before-context-after-v1",
            b"whole-case-atomic-v1",
            &usize_bytes(DEFAULT_EXPLORE_STEP_LIMIT),
            &usize_bytes(DEFAULT_EXPLORE_COLLECTION_LIMIT),
        ]),
        mechanism_observation_identity_digest(
            mechanism_request.map(|request| request.id.digest_bytes()),
        ),
        retention_authorization_digest(report_request),
        schemas,
    );
    let replay_closure = RequiredObligationId::new(contract_digest(&[REPLAY_CLOSURE_CONTRACT_V1]));
    let nonce = ExploreRunNonce::new(nonce).map_err(|error| error.to_string())?;
    let header = ExploreRunHeader::new(identity, case_universe, [replay_closure], nonce)
        .map_err(|error| error.to_string())?;
    Ok(PreparedExactStreamHeader {
        header,
        replay_closure,
    })
}

fn mechanism_observation_identity_digest(checked_request_id: Option<[u8; 32]>) -> CanonicalDigest {
    match checked_request_id {
        None => contract_digest(&[b"mechanism-observation", b"deferred"]),
        Some(checked_request_id) => contract_digest(&[
            b"mechanism-observation",
            b"enabled-v1",
            b"checked-request-id",
            &checked_request_id,
            b"batch-codec-and-resource-contract",
            &mechanism_stream_contract_digest_v1(),
        ]),
    }
}

fn report_request_digest(request: ExploreReportRequest) -> CanonicalDigest {
    let case_graph = match request.case_graph {
        ExploreCaseGraphRequest::Omit => b"case-graph-omitted".as_slice(),
        ExploreCaseGraphRequest::Include => b"case-graph-full".as_slice(),
    };
    let ledger = match request.ledger {
        ExploreLedgerRequest::Omit => b"ledger-omitted".as_slice(),
        ExploreLedgerRequest::MatchingConfigurations => {
            b"ledger-matching-configurations".as_slice()
        }
    };
    contract_digest(&[
        b"report-request-v3",
        b"projected-rows",
        case_graph,
        ledger,
        b"mechanisms-deferred",
        b"bounded-canonical-raw-group-preview-v1",
        b"inspectable-configuration-manifest-v3",
    ])
}

fn retention_authorization_digest(request: ExploreReportRequest) -> CanonicalDigest {
    let case_graph = match request.case_graph {
        ExploreCaseGraphRequest::Omit => b"ordinal-case-classification-omitted".as_slice(),
        ExploreCaseGraphRequest::Include => b"ordinal-case-classification-graph-full".as_slice(),
    };
    let ledger = match request.ledger {
        ExploreLedgerRequest::Omit => b"matching-ledger-omitted".as_slice(),
        ExploreLedgerRequest::MatchingConfigurations => b"matching-ledger-full".as_slice(),
    };
    contract_digest(&[
        b"retention-v3",
        b"projected-results",
        b"configuration-manifest-v3",
        b"globally-bounded-value-disclosure",
        &usize_bytes(CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2),
        &usize_bytes(CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2),
        case_graph,
        ledger,
    ])
}

fn parse_checked_digest(field: &'static str, value: &str) -> Result<CanonicalDigest, String> {
    CanonicalDigest::from_lowercase_sha256(field, value).map_err(|error| error.to_string())
}

fn usize_bytes(value: usize) -> [u8; 16] {
    (value as u128).to_le_bytes()
}

fn contract_digest(segments: &[&[u8]]) -> CanonicalDigest {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN_V1);
    for segment in segments {
        hasher.update((segment.len() as u64).to_le_bytes());
        hasher.update(segment);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    CanonicalDigest::from_lowercase_sha256("stream_contract", &encoded)
        .expect("SHA-256 encoding is canonical")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_mechanism_binding_has_an_explicit_deferred_identity() {
        assert_eq!(
            mechanism_observation_identity_digest(None).to_lowercase_hex(),
            "4a9856309bbfe75824577e9518b222af8f998581e1da1db8fa31414f50b3ab11"
        );
    }

    #[test]
    fn enabled_mechanism_binding_commits_to_the_checked_request() {
        let left = mechanism_observation_identity_digest(Some([1; 32]));
        let right = mechanism_observation_identity_digest(Some([2; 32]));
        assert_ne!(left, right);
        assert_ne!(left, mechanism_observation_identity_digest(None));
    }
}
