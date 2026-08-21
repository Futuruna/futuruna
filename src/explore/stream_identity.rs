//! Producer-owned identity for one durable exact Explore stream.
//!
//! This module is deliberately smaller than the execution coordinator.  It
//! consumes only a validated checked-query view and immutable report/evaluator
//! contracts, then binds the complete case universe before any probe, proof or
//! scheduling decision is allowed to run.

use sha2::{Digest, Sha256};

use super::classification_regions::SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1;
use super::exact_stream::{
    EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_JSON_BYTE_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_SEMANTIC_BYTE_LIMIT_V1,
    EXACT_OBSERVABLE_RESULT_PREVIEW_VALUE_NODE_LIMIT_V1,
};
use super::report::{DEFAULT_EXPLORE_COLLECTION_LIMIT, DEFAULT_EXPLORE_STEP_LIMIT};
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
) -> Result<PreparedExactStreamHeader, String> {
    let checked = artifacts
        .checked_exploration_query(accepted_query_index)
        .map_err(|error| format!("cannot bind checked Explore stream identity: {error:?}"))?;

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
        ]),
        contract_digest(&[
            b"semantic-evidence",
            b"normalized-merkle-treap-v2",
            b"persistent-case-support-treap-v2",
        ]),
        contract_digest(&[
            b"exact-snapshot",
            b"exact-observable-snapshot-v4",
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
            b"inspectable-configuration-manifest-v2",
            b"configuration-value-node-limit",
            &usize_bytes(CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2),
            b"configuration-value-semantic-byte-limit",
            &usize_bytes(CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2),
            b"replay-derived-source-probe-progress-v1",
            b"inspectable-source-probe-limits-v1",
        ]),
        contract_digest(&[
            b"terminal-result",
            b"exact-report-v3",
            b"grouped-having-filter-v1",
            b"full-result-publication-required-v1",
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
        contract_digest(&[
            b"report-request",
            b"baseline-projected-rows",
            b"ledger-omitted",
            b"mechanisms-deferred",
            b"bounded-canonical-raw-group-preview-v1",
            b"inspectable-configuration-manifest-v2",
        ]),
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
            b"exact-interpreter-v1",
            b"whole-case-atomic-v1",
            &usize_bytes(DEFAULT_EXPLORE_STEP_LIMIT),
            &usize_bytes(DEFAULT_EXPLORE_COLLECTION_LIMIT),
        ]),
        contract_digest(&[b"mechanism-observation", b"deferred"]),
        contract_digest(&[
            b"retention",
            b"projected-results",
            b"configuration-manifest-v2",
            b"globally-bounded-value-disclosure",
            &usize_bytes(CONFIGURATION_MANIFEST_VALUE_NODE_LIMIT_V2),
            &usize_bytes(CONFIGURATION_MANIFEST_VALUE_SEMANTIC_BYTE_LIMIT_V2),
        ]),
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
