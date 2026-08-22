//! Crash-safe synchronous coordinator for one durable exact Explore stream.
//!
//! This is the owner-local mint boundary between checked Explore semantics,
//! immutable storage and the two pure reducers.  It deliberately has one
//! in-process evaluator and no worker/thread policy.  Every mutating path is:
//!
//! 1. validate and prepare the complete semantic transition,
//! 2. install any content-addressed body,
//! 3. append the canonical journal record,
//! 4. apply the prepared run/exact reducer states, and only then
//! 5. expose the new cursor or snapshot.
//!
//! Opening an existing stream verifies its checked genesis identity, every
//! historical writer fence, every canonical record envelope, and all exact
//! evidence blobs before acquiring a fresh writer generation.  Canonical
//! batches previously minted by this owner-local coordinator cross a distinct
//! restore seam only after their blob, envelope, fence, support and normalized
//! facts all agree.  Restart does not repeat prior evaluator/proof work.  Fresh
//! or remote proposals still require ordinary evaluator/proof validation.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU64};
use std::path::Path;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io::Read;

use sha2::{Digest, Sha256};

use super::case_graph::{
    lower_case_terminal_rank_runs, CaseOpenReason, CaseRankRunLoweringError,
    CaseRankRunLoweringResource, CaseTerminal, CaseTerminalRankRun, DEFAULT_MAX_CASE_RANK_RUNS,
    DEFAULT_MAX_CASE_RANK_RUN_AXES,
};
use super::exact::{
    seal_local_evaluator_observation_batch_v1, ExactStreamCaseAttempt, ExactStreamEvaluator,
    ExactStreamEvaluatorPrepareError,
};
use super::exact_stream::{
    decode_exact_case_observation_batch_v1, decode_exact_closed_region_batch_v1,
    encode_exact_case_observation_batch_v1, encode_exact_closed_region_batch_v1,
    restore_coordinator_committed_observation_batch_v1,
    restore_coordinator_committed_region_batch_v1, ExactCaseObservationBatchProposalV1,
    ExactCaseObservationProposalV1, ExactClosedClassificationSupportsV1,
    ExactClosedClassificationV1, ExactClosedRegionBatchProposalV1, ExactEvidenceReducer,
    ExactEvidenceSnapshotV1, ExactProjectionShapeV1, ExactRepresentativePolicyV1,
    ValidatedExactCaseObservationBatchV1, ValidatedExactClosedRegionBatchV1,
};
use super::mechanism::{
    CheckedMechanismObservationRequestV1, MechanismObservedEvidence, MechanismQueryId,
};
use super::mechanism_snapshot::{
    render_mechanism_observable_checkpoint_json_line_v1,
    render_mechanism_observable_checkpoint_unavailable_json_line_v1,
    MechanismObservableCheckpointMetadataV1, MECHANISM_OBSERVABLE_CHECKPOINT_BLOB_KIND_V1,
    MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_BLOB_KIND_V1,
};
use super::mechanism_stream::{
    decode_mechanism_observation_batch_v1, encode_mechanism_observation_batch_v1,
    restore_committed_mechanism_batch_v1, MechanismBinAssignmentOutcomeV1,
    MechanismCaseObservationOutcomeProposalV1, MechanismEvidenceReducerV1,
    MechanismPermanentUntracedReasonV1, ValidatedMechanismObservationBatchV1,
    MAX_NORMALIZED_SEMANTIC_FACTS_PER_BATCH, MECHANISM_OBSERVATION_BLOB_KIND_V1,
};
use super::report::{
    ExploreCaseGraphRequest, ExploreReportRequest, ExploreStopReason,
    DEFAULT_EXPLORE_COLLECTION_LIMIT, DEFAULT_EXPLORE_STEP_LIMIT,
};
use super::run_store::RunStoreLimits;
use super::run_stream::{
    CanonicalDigest, CanonicalRunRecordPayload, CoveragePlan, DiscoveryEventKind, ExactCaseSupport,
    ExploreRunCursor, ExploreRunHeader, ExploreRunId, ExploreRunStream, ExploreWriterId,
    FencedWriterLease, FrontierEvidenceKind, ObservationEvidenceKind, PauseReason,
    PreparedRunTransition, RequiredFrontier, RequiredObligationId, RunLifecycle,
    SemanticEvidenceFact, SemanticEvidenceLayer, SemanticEvidenceSubject, TerminalMethodHash,
    TerminalPayloadHash, TerminalSealKind,
};
use super::run_stream_codec::{decode_genesis_record, decode_later_record, encode_record};
use super::run_stream_store::{ExploreRunStreamStore, ExploreWriterFenceReceipt};
use super::source_proof_plan::SourceProofPlan;
use super::stream_identity::{
    prepare_exact_stream_header, prepare_exact_stream_header_with_mechanism,
};
use super::stream_probe::{
    decode_source_probe_manifest_v1, encode_source_probe_manifest_v1, ExactSourceProbeManifestV1,
    ExactSourceProbePhaseV1, ExactSourceProbeProgressV1, SOURCE_PROBE_MANIFEST_BLOB_KIND_V1,
};
use super::stream_proof::{
    decode_source_proof_candidate_ranks_v1, encode_source_proof_candidate_ranks_v1,
    prepare_source_proof_exact_coverage_v1,
};
use super::stream_replay::{
    decode_exact_replay_closure_manifest_v1, encode_exact_replay_closure_manifest_v1,
    exact_replay_witness_ranks_v1, validate_exact_replay_closure_v1, ExactReplayClosureManifestV1,
    EXACT_REPLAY_CLOSURE_BLOB_KIND_V1,
};
use super::stream_resource::ExactStreamSnapshotPublicationAuthority;
use super::stream_snapshot::{
    render_exact_observable_snapshot_json_line_v1,
    render_exact_observable_snapshot_unavailable_json_line_v1,
    render_exact_semantic_answer_json_v1, ExactCaseGraphPublicationResourceV1,
    ExactObservableSnapshotMetadataV1, ExactPreparedCaseGraphPublicationV1,
    ExactSemanticAnswerMetadataV1, EXACT_OBSERVABLE_SNAPSHOT_BLOB_KIND_V1,
    EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_BLOB_KIND_V1, EXACT_SEMANTIC_ANSWER_BLOB_KIND_V1,
};
use super::ExploreQueryIr;
use crate::{ExploreRepresentative, Stmt, TypeCheckArtifacts};

const WRITER_FENCE_IDENTITY_V1: &[u8] = b"futuruna.explore.coordinator-writer-fence-identity.v1";
const EXACT_OBSERVATION_BLOB_V1: &str = "exact-observations-v1";
const EXACT_REGION_BLOB_V1: &str = "exact-regions-v1";
const SOURCE_CANDIDATE_BLOB_V1: &str = "source-candidates-v1";
const SOURCE_PROBE_FALLBACK_PROOF_SET_V1: &[u8] =
    b"futuruna.explore.exact-stream.source-probe-fallback.v1";
const EXACT_STREAM_COMPLETED_EXACT_EXHAUSTION_METHOD_V1: &[u8] =
    b"futuruna.explore.exact-stream.completed-exact-exhaustion.v1";
/// Keep the retained replay proposal set comfortably below the 64 MiB wire
/// ceiling so finalization cannot allocate an unbounded manifest before the
/// codec gets a chance to reject it.
const EXACT_STREAM_ATOMIC_REPLAY_ACCUMULATION_BUDGET_V1: usize = 32 * 1024 * 1024;

/// The first coordinator generation deliberately keeps an admitted atomic
/// work unit small enough for frequent resource and deadline boundaries.
pub(super) const EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP: u16 = 256;

/// Soft body target below the exact observation codec's 16 MiB hard bound.
/// A single individually encodable observation may exceed this target; it is
/// then committed alone, with the canonical batch encoder retaining final
/// authority over the hard wire limit.
const EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1: usize = 8 * 1024 * 1024;

/// A coordinator failure is intentionally contextual rather than a large
/// public sum type: the underlying store/codec/reducer errors already carry
/// the precise invariant which failed.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ExactStreamCoordinatorError {
    message: Box<str>,
    snapshot_publication_capacity: bool,
    mechanism_fixed_capacity: bool,
}

impl ExactStreamCoordinatorError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into().into_boxed_str(),
            snapshot_publication_capacity: false,
            mechanism_fixed_capacity: false,
        }
    }

    fn context(context: &str, error: impl fmt::Display) -> Self {
        Self::invalid(format!("{context}: {error}"))
    }

    fn snapshot_capacity(context: &str, error: impl fmt::Display) -> Self {
        Self {
            message: format!("{context}: {error}").into_boxed_str(),
            snapshot_publication_capacity: true,
            mechanism_fixed_capacity: false,
        }
    }

    fn mechanism_capacity(context: &str, error: impl fmt::Display) -> Self {
        Self {
            message: format!("{context}: {error}").into_boxed_str(),
            snapshot_publication_capacity: false,
            mechanism_fixed_capacity: true,
        }
    }

    const fn is_snapshot_publication_capacity(&self) -> bool {
        self.snapshot_publication_capacity
    }

    pub(super) const fn is_mechanism_fixed_capacity(&self) -> bool {
        self.mechanism_fixed_capacity
    }
}

impl fmt::Display for ExactStreamCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ExactStreamCoordinatorError {}

#[derive(Debug)]
enum ExactEvaluatorEnsureError {
    OperationalLimit(ExploreStopReason),
    Failure(ExactStreamCoordinatorError),
}

/// One deliberately small scheduler step.  An operationally open case is not
/// committed at all, so the same CaseId remains an atomic retry unit.
#[derive(Debug)]
pub(super) enum ExactStreamAdvance {
    ClassificationClosedFinalizationPending,
    Committed {
        rank: u128,
        closed_case_count: u128,
    },
    CaseOpen {
        rank: u128,
        reason: ExploreStopReason,
    },
}

/// Why a committed bounded classification block stopped growing. Every rank
/// named here remains open unless `ClassificationClosedFinalizationPending`
/// is selected.
#[derive(Debug)]
pub(super) enum ExactStreamBatchStop {
    CaseCapReached {
        next_rank: u128,
    },
    ByteTargetReached {
        next_rank: u128,
    },
    CaseOpen {
        rank: u128,
        reason: ExploreStopReason,
    },
    ClassificationClosedFinalizationPending,
}

/// One candidate-first, whole-CaseId block advancement. A completed prefix is
/// committed in one journal event even when the next CaseId remains
/// operationally open; if no prefix exists, no semantic state is changed.
#[derive(Debug)]
pub(super) enum ExactStreamBatchAdvance {
    ClassificationClosedFinalizationPending,
    Committed {
        ranks: Box<[u128]>,
        canonical_blob_bytes: usize,
        closed_case_count: u128,
        stop: ExactStreamBatchStop,
    },
    CaseOpen {
        rank: u128,
        reason: ExploreStopReason,
    },
}

/// Why one bounded source-probe candidate block stopped. Unlike the residual
/// scheduler, every rank here comes from the durable prepared manifest.
#[derive(Debug)]
pub(super) enum ExactProbeCandidateBatchStop {
    CaseCapReached {
        next_rank: u128,
    },
    ByteTargetReached {
        next_rank: u128,
    },
    CaseOpen {
        rank: u128,
        reason: ExploreStopReason,
    },
    CandidatesComplete,
}

#[derive(Debug)]
pub(super) enum ExactProbeCandidateBatchAdvance {
    CandidatesComplete,
    Committed {
        ranks: Box<[u128]>,
        canonical_blob_bytes: usize,
        closed_case_count: u128,
        stop: ExactProbeCandidateBatchStop,
    },
    CaseOpen {
        rank: u128,
        reason: ExploreStopReason,
    },
}

#[derive(Debug)]
pub(super) enum ExactReplayClosureAdvance {
    AlreadyClosed,
    Closed {
        witness_count: usize,
        normalized_witness_digest: CanonicalDigest,
    },
    WitnessOpen {
        rank: u128,
        reason: ExploreStopReason,
    },
    LimitReached {
        detail: String,
    },
}

/// Authenticated pointer to the latest canonical terminal answer publication.
///
/// The raw blob digest is the immutable store address. The domain-separated
/// payload hash is the semantic commitment consumed by `TerminalSeal`. Keeping
/// both values together makes publication/seal recovery idempotent without
/// searching backwards through the journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExactTerminalPublicationReceiptV1 {
    blob_digest: CanonicalDigest,
    payload_hash: TerminalPayloadHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ExactTerminalPublicationAdvanceV1 {
    Published(ExactTerminalPublicationReceiptV1),
    LimitReached { phase: &'static str, detail: String },
}

/// Opaque coordinator-minted case-view materialization bound to the current
/// monotone classification state and immutable report request. Publication
/// hooks accept this token rather than a caller-constructible graph enum, so a
/// same-count but rank-permuted DAG cannot cross the checked byte seam.
pub(super) struct PreparedExactCaseGraphPublication {
    publication: ExactPreparedCaseGraphPublicationV1,
    run_id: ExploreRunId,
    report_request: ExploreReportRequest,
    closed_case_count: u128,
    classification_support_identity_hashes: [CanonicalDigest; 4],
}

impl PreparedExactCaseGraphPublication {
    pub(super) const fn publication(&self) -> &ExactPreparedCaseGraphPublicationV1 {
        &self.publication
    }
}

/// Opaque canonical checkpoint bytes minted from one exact running cursor.
/// Callers can inspect and eventually move the bytes, but cannot construct a
/// token that bypasses the coordinator-owned encoder.
pub(super) struct PreparedExactObservableSnapshotPublication {
    cursor: ExploreRunCursor,
    canonical_json_line: Vec<u8>,
    probe_milestone_complete: bool,
    closed_case_count: u128,
    kind: PreparedExactObservableSnapshotPublicationKind,
}

/// Opaque canonical mechanism-checkpoint bytes bound to the current running
/// cursor. As with exact snapshots, the content is installed before the
/// journal pointer and never changes semantic evidence.
pub(super) struct PreparedMechanismObservableCheckpointPublicationV1 {
    cursor: ExploreRunCursor,
    canonical_json_line: Vec<u8>,
    kind: PreparedMechanismObservableCheckpointPublicationKindV1,
}

#[derive(Clone, Copy)]
enum PreparedMechanismObservableCheckpointPublicationKindV1 {
    Included,
    CapacityUnavailable,
}

impl PreparedMechanismObservableCheckpointPublicationV1 {
    pub(super) const fn cursor(&self) -> ExploreRunCursor {
        self.cursor
    }

    pub(super) fn canonical_json_line(&self) -> &[u8] {
        &self.canonical_json_line
    }

    pub(super) const fn materialization_capacity_detail(&self) -> Option<&'static str> {
        match self.kind {
            PreparedMechanismObservableCheckpointPublicationKindV1::Included => None,
            PreparedMechanismObservableCheckpointPublicationKindV1::CapacityUnavailable => {
                Some("mechanism checkpoint exceeded its bounded canonical rendering envelope")
            }
        }
    }

    pub(super) fn into_canonical_json_line(self) -> Vec<u8> {
        self.canonical_json_line
    }
}

enum PreparedExactObservableSnapshotPublicationKind {
    Included,
    CapacityUnavailable { detail: Box<str> },
}

impl PreparedExactObservableSnapshotPublication {
    pub(super) const fn cursor(&self) -> ExploreRunCursor {
        self.cursor
    }

    pub(super) fn canonical_json_line(&self) -> &[u8] {
        &self.canonical_json_line
    }

    pub(super) const fn probe_milestone_complete(&self) -> bool {
        self.probe_milestone_complete
    }

    pub(super) const fn closed_case_count(&self) -> u128 {
        self.closed_case_count
    }

    pub(super) fn materialization_capacity_detail(&self) -> Option<&str> {
        match &self.kind {
            PreparedExactObservableSnapshotPublicationKind::Included => None,
            PreparedExactObservableSnapshotPublicationKind::CapacityUnavailable { detail } => {
                Some(detail)
            }
        }
    }

    pub(super) fn into_canonical_json_line(self) -> Vec<u8> {
        self.canonical_json_line
    }
}

impl ExactTerminalPublicationReceiptV1 {
    pub(super) const fn blob_digest(self) -> CanonicalDigest {
        self.blob_digest
    }

    pub(super) const fn payload_hash(self) -> TerminalPayloadHash {
        self.payload_hash
    }
}

/// Durable single-writer coordinator.  The held store owns the directory lock;
/// the optional receipt/lease pair is the only authority to append records.
pub(super) struct ExactStreamCoordinator<'a> {
    store: ExploreRunStreamStore,
    stream: ExploreRunStream,
    exact: ExactEvidenceReducer,
    /// Present only when the stream identity admits durable endpoint-observer
    /// evidence. Exact classification can run without that later replay layer.
    mechanism: Option<MechanismEvidenceReducerV1>,
    mechanism_request: Option<CheckedMechanismObservationRequestV1>,
    evaluator: Option<ExactStreamEvaluator<'a>>,
    statements: &'a [Stmt],
    source_dir: Option<&'a str>,
    artifacts: &'a TypeCheckArtifacts,
    accepted_query_index: usize,
    query: &'a ExploreQueryIr,
    report_request: ExploreReportRequest,
    replay_closure: RequiredObligationId,
    writer_fence: Option<ExploreWriterFenceReceipt>,
    active_lease: Option<FencedWriterLease>,
    candidate_ranks: BTreeSet<u128>,
    source_probe_manifest_blob: Option<CanonicalDigest>,
    source_probe_manifest: Option<ExactSourceProbeManifestV1>,
    source_proof_set_id: Option<CanonicalDigest>,
    source_proof_completed: Option<CanonicalDigest>,
    published_terminal_result: Option<ExactTerminalPublicationReceiptV1>,
    /// Replay-derived materialized-view debt. A pause not immediately preceded
    /// by snapshot publication sets it; Resume/Recovery and crashes preserve
    /// it, and only a committed full-snapshot or snapshot-unavailable observer
    /// publication clears it. The next invocation services the debt before
    /// dispatching more semantic work.
    pending_observable_snapshot_on_resume: bool,
    /// Whether the immediately preceding committed record was a full snapshot
    /// or snapshot-unavailable observer publication. A following pause uses
    /// this to derive live debt exactly as replay does.
    last_committed_record_serviced_snapshot_view: bool,
}

impl<'a> ExactStreamCoordinator<'a> {
    /// Create a new stream or fully replay the immutable stream already in
    /// `directory`.  This function does not evaluate any unexplored CaseId.
    /// Existing exact evidence is restored from its authenticated local
    /// journal/blob commitment; prior CaseIds and proofs are not recomputed.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn open_or_create(
        directory: impl AsRef<Path>,
        limits: RunStoreLimits,
        statements: &'a [Stmt],
        source_dir: Option<&'a str>,
        artifacts: &'a TypeCheckArtifacts,
        accepted_query_index: usize,
        report_request: ExploreReportRequest,
    ) -> Result<Self, ExactStreamCoordinatorError> {
        Self::open_or_create_internal(
            directory,
            limits,
            statements,
            source_dir,
            artifacts,
            accepted_query_index,
            report_request,
            None,
        )
    }

    /// Dormant integration seam for a stream whose sequence-zero identity
    /// explicitly authorizes mechanism incidence. Runtime trace minting and
    /// public mechanism rendering remain separate future steps; this method
    /// only joins already checked requests and validated batches to the
    /// authenticated journal/replay machinery.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn open_or_create_with_mechanism(
        directory: impl AsRef<Path>,
        limits: RunStoreLimits,
        statements: &'a [Stmt],
        source_dir: Option<&'a str>,
        artifacts: &'a TypeCheckArtifacts,
        accepted_query_index: usize,
        report_request: ExploreReportRequest,
        mechanism_request: CheckedMechanismObservationRequestV1,
    ) -> Result<Self, ExactStreamCoordinatorError> {
        Self::open_or_create_internal(
            directory,
            limits,
            statements,
            source_dir,
            artifacts,
            accepted_query_index,
            report_request,
            Some(mechanism_request),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn open_or_create_internal(
        directory: impl AsRef<Path>,
        limits: RunStoreLimits,
        statements: &'a [Stmt],
        source_dir: Option<&'a str>,
        artifacts: &'a TypeCheckArtifacts,
        accepted_query_index: usize,
        report_request: ExploreReportRequest,
        mechanism_request: Option<CheckedMechanismObservationRequestV1>,
    ) -> Result<Self, ExactStreamCoordinatorError> {
        let checked = artifacts
            .checked_exploration_query(accepted_query_index)
            .map_err(|error| {
                ExactStreamCoordinatorError::invalid(format!(
                    "cannot select checked Explore query for stream coordination: {error:?}"
                ))
            })?;
        if let Some(request) = mechanism_request.as_ref() {
            artifacts
                .validate_checked_runtime_entry_v1(statements, source_dir)
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot authorize mechanism observation source snapshot",
                        error,
                    )
                })?;
            validate_mechanism_request_for_checked_query(request, &checked)?;
        }
        let query = checked.closed_query;
        let mut exact = exact_reducer_for_query(query)?;
        let mut mechanism = mechanism_request
            .as_ref()
            .map(|request| MechanismEvidenceReducerV1::new(request.clone()))
            .transpose()
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot initialize mechanism evidence reducer",
                    error,
                )
            })?;
        let mut store =
            ExploreRunStreamStore::open_or_create(directory, limits).map_err(|error| {
                ExactStreamCoordinatorError::context("cannot open run store", error)
            })?;
        let genesis = store
            .read_genesis()
            .map_err(|error| ExactStreamCoordinatorError::context("cannot read genesis", error))?;

        match genesis {
            None => {
                let nonce = os_random_nonzero_digest("run nonce")?;
                let prepared_header = match mechanism_request.as_ref() {
                    None => prepare_exact_stream_header(
                        artifacts,
                        accepted_query_index,
                        nonce,
                        report_request,
                    ),
                    Some(request) => prepare_exact_stream_header_with_mechanism(
                        artifacts,
                        accepted_query_index,
                        nonce,
                        report_request,
                        Some(request),
                    ),
                }
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot prepare checked stream header",
                        error,
                    )
                })?;
                require_reducer_universe(&exact, &prepared_header.header)?;

                let writer_id = ExploreWriterId::new(os_random_nonzero_digest("writer id")?);
                let generation = store.next_writer_fence_generation().ok_or_else(|| {
                    ExactStreamCoordinatorError::invalid(
                        "initial writer-fence generation space is exhausted",
                    )
                })?;
                if generation.get() != 1 {
                    return Err(ExactStreamCoordinatorError::invalid(format!(
                        "empty run store proposed initial writer generation {} instead of 1",
                        generation.get()
                    )));
                }
                let fence_identity = canonical_writer_fence_identity(
                    prepared_header.header.run_id(),
                    generation,
                    writer_id,
                );
                let prepared_fence = store
                    .prepare_initial_writer_fence(&fence_identity)
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot prepare initial writer fence",
                            error,
                        )
                    })?;
                if prepared_fence.generation() != generation
                    || prepared_fence.writer_lease_identity() != fence_identity.as_slice()
                {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "run store returned inconsistent initial writer-fence material",
                    ));
                }
                let receipt_hash = CanonicalDigest::from_lowercase_sha256(
                    "writer_fence_receipt",
                    prepared_fence.receipt_hash(),
                )
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "initial writer-fence receipt is not canonical",
                        error,
                    )
                })?;
                let lease = FencedWriterLease::new(
                    prepared_header.header.run_id(),
                    generation,
                    writer_id,
                    receipt_hash,
                );
                let opened = ExploreRunStream::prepare_open(prepared_header.header.clone(), lease)
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot prepare RunOpened transition",
                            error,
                        )
                    })?;
                let genesis_bytes =
                    encode_record(opened.event(), opened.payload()).map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot encode RunOpened record",
                            error,
                        )
                    })?;
                let receipt = store
                    .install_genesis(prepared_fence, &genesis_bytes)
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot durably install RunOpened",
                            error,
                        )
                    })?;
                require_receipt_matches_lease(&receipt, lease, &fence_identity)?;
                let stream = opened.install_committed();

                Ok(Self {
                    store,
                    stream,
                    exact,
                    mechanism,
                    mechanism_request,
                    evaluator: None,
                    statements,
                    source_dir,
                    artifacts,
                    accepted_query_index,
                    query,
                    report_request,
                    replay_closure: prepared_header.replay_closure,
                    writer_fence: Some(receipt),
                    active_lease: Some(lease),
                    candidate_ranks: BTreeSet::new(),
                    source_probe_manifest_blob: None,
                    source_probe_manifest: None,
                    source_proof_set_id: None,
                    source_proof_completed: None,
                    published_terminal_result: None,
                    pending_observable_snapshot_on_resume: false,
                    last_committed_record_serviced_snapshot_view: false,
                })
            }
            Some(genesis_bytes) => {
                let decoded = decode_genesis_record(&genesis_bytes).map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot decode canonical RunOpened record",
                        error,
                    )
                })?;
                let (genesis_event, genesis_payload) = decoded.into_parts();
                let (stored_header, genesis_lease) = match &genesis_payload {
                    CanonicalRunRecordPayload::RunOpened { header, lease } => {
                        (header.clone(), *lease)
                    }
                    _ => {
                        return Err(ExactStreamCoordinatorError::invalid(
                            "genesis decoder returned a non-RunOpened payload",
                        ))
                    }
                };
                verify_historical_lease(&store, genesis_lease)?;

                let expected = match mechanism_request.as_ref() {
                    None => prepare_exact_stream_header(
                        artifacts,
                        accepted_query_index,
                        stored_header.nonce().identity(),
                        report_request,
                    ),
                    Some(request) => prepare_exact_stream_header_with_mechanism(
                        artifacts,
                        accepted_query_index,
                        stored_header.nonce().identity(),
                        report_request,
                        Some(request),
                    ),
                }
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot reconstruct checked stream header",
                        error,
                    )
                })?;
                if expected.header != stored_header {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "stored Explore stream header does not match the current checked program, query, domain, report, evaluator and schema identities",
                    ));
                }
                require_reducer_universe(&exact, &stored_header)?;

                let mut stream = ExploreRunStream::replay_open(genesis_payload, &genesis_event)
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "RunOpened reducer replay failed",
                            error,
                        )
                    })?;
                let mut candidate_ranks = BTreeSet::new();
                let mut source_probe_manifest_blob = None;
                let mut source_probe_manifest = None;
                let mut source_proof_set_id = None;
                let mut source_proof_completed = None;
                let mut published_terminal_result = None;
                let mut previous_record_serviced_snapshot_view = false;
                let mut observable_snapshot_debt = false;

                let replay = store.replay_events().map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot enumerate immutable stream records",
                        error,
                    )
                })?;
                for raw in replay {
                    let raw = raw.map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot read immutable stream record",
                            error,
                        )
                    })?;
                    let decoded =
                        decode_later_record(&raw.bytes, stream.header()).map_err(|error| {
                            ExactStreamCoordinatorError::context(
                                "cannot decode canonical stream record",
                                error,
                            )
                        })?;
                    let (event, payload) = decoded.into_parts();
                    let services_snapshot_view = matches!(
                        &payload,
                        CanonicalRunRecordPayload::Discovery {
                            kind: DiscoveryEventKind::SnapshotPublished
                                | DiscoveryEventKind::SnapshotUnavailablePublished,
                            ..
                        }
                    );
                    if matches!(&payload, CanonicalRunRecordPayload::Paused { .. }) {
                        // A mechanism checkpoint is not defined before its
                        // source-probe milestone. Such a pause is already a
                        // complete journal resume point, not deferred observer
                        // work that could have been published at that cursor.
                        let mechanism_checkpoint_not_yet_available =
                            mechanism_request.is_some() && source_proof_completed.is_none();
                        observable_snapshot_debt = !previous_record_serviced_snapshot_view
                            && !mechanism_checkpoint_not_yet_available;
                    } else if services_snapshot_view {
                        observable_snapshot_debt = false;
                    }
                    if event.sequence() != raw.sequence
                        || event.journal_head().to_lowercase_hex() != raw.journal_head.as_ref()
                    {
                        return Err(ExactStreamCoordinatorError::invalid(format!(
                            "immutable record name disagrees with decoded envelope at sequence {}",
                            raw.sequence
                        )));
                    }
                    verify_historical_lease(&store, payload.lease())?;

                    if mechanism_request.is_some()
                        && matches!(
                            &payload,
                            CanonicalRunRecordPayload::Discovery {
                                kind: DiscoveryEventKind::TerminalResultPublished,
                                ..
                            } | CanonicalRunRecordPayload::TerminalSeal { .. }
                        )
                    {
                        return Err(ExactStreamCoordinatorError::invalid(
                            "mechanism-enabled stream contains an exact-only terminal publication or seal",
                        ));
                    }

                    if matches!(
                        &payload,
                        CanonicalRunRecordPayload::SemanticObservation {
                            producer_kind: ObservationEvidenceKind::MechanismObserved,
                            ..
                        }
                    ) {
                        synchronize_mechanism_target_knowledge(&exact, mechanism.as_mut(), false)?;
                    }
                    apply_exact_replay(
                        &store,
                        &mut exact,
                        stream.header(),
                        stream.frontier(),
                        expected.replay_closure,
                        &payload,
                    )?;
                    apply_mechanism_replay(
                        &store,
                        mechanism.as_mut(),
                        mechanism_request.as_ref(),
                        &stored_header,
                        &payload,
                    )?;
                    let mut staged_candidates = None;
                    let mut staged_manifest_blob = source_probe_manifest_blob;
                    let mut staged_manifest = source_probe_manifest;
                    let mut staged_proof_set = source_proof_set_id;
                    let mut staged_proof_completed = source_proof_completed;
                    let mut staged_terminal_result = published_terminal_result;
                    match &payload {
                        CanonicalRunRecordPayload::Discovery {
                            kind: DiscoveryEventKind::ProbePlanPrepared,
                            canonical_discovery_hash,
                            ..
                        } => {
                            let (manifest, ranks) = read_source_probe_manifest(
                                &store,
                                &stored_header,
                                *canonical_discovery_hash,
                            )?;
                            if staged_manifest_blob
                                .is_some_and(|prior| prior != *canonical_discovery_hash)
                                || staged_manifest.is_some_and(|prior| prior != manifest)
                            {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "stream contains conflicting prepared source-probe manifests",
                                ));
                            }
                            if staged_proof_set.is_some() || staged_proof_completed.is_some() {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "source-probe manifest was prepared after coverage or completion",
                                ));
                            }
                            staged_manifest_blob = Some(*canonical_discovery_hash);
                            staged_manifest = Some(manifest);
                            staged_candidates = Some(ranks);
                        }
                        CanonicalRunRecordPayload::Discovery {
                            kind: DiscoveryEventKind::CandidateDiscovered,
                            canonical_discovery_hash,
                            ..
                        } => {
                            if staged_manifest.is_none()
                                || staged_proof_set.is_some()
                                || staged_proof_completed.is_some()
                            {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "candidate discovery is outside the prepared pre-coverage probe phase",
                                ));
                            }
                            let blob = store
                                .read_blob(
                                    SOURCE_CANDIDATE_BLOB_V1,
                                    &canonical_discovery_hash.to_lowercase_hex(),
                                )
                                .map_err(|error| {
                                    ExactStreamCoordinatorError::context(
                                        "cannot read candidate discovery blob",
                                        error,
                                    )
                                })?;
                            let ranks = decode_source_proof_candidate_ranks_v1(
                                &blob,
                                stored_header.case_universe().case_count(),
                            )
                            .map_err(|error| {
                                ExactStreamCoordinatorError::context(
                                    "cannot decode candidate discovery blob",
                                    error,
                                )
                            })?;
                            if let Some(manifest) = staged_manifest {
                                if manifest.candidate_blob() != *canonical_discovery_hash
                                    || manifest.candidate_count() != ranks.len() as u128
                                {
                                    return Err(ExactStreamCoordinatorError::invalid(
                                        "candidate discovery disagrees with its prepared source-probe manifest",
                                    ));
                                }
                            }
                            let mut next = candidate_ranks.clone();
                            next.extend(ranks.iter().copied());
                            staged_candidates = Some(next);
                        }
                        CanonicalRunRecordPayload::Discovery {
                            kind: DiscoveryEventKind::ProbePlanCompleted,
                            canonical_discovery_hash,
                            ..
                        } => {
                            if staged_proof_set != Some(*canonical_discovery_hash) {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "source-proof completion marker does not follow its accepted coverage plan",
                                ));
                            }
                            if staged_manifest.is_none_or(|manifest| {
                                manifest.proof_set_id() != *canonical_discovery_hash
                            }) {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "source-probe completion marker has no matching prepared manifest",
                                ));
                            }
                            if candidate_ranks
                                .iter()
                                .any(|rank| stream.frontier().open_cases().contains_rank(*rank))
                            {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "source-probe completion marker leaves discovered candidates open",
                                ));
                            }
                            if staged_proof_completed
                                .is_some_and(|prior| prior != *canonical_discovery_hash)
                            {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "stream contains conflicting completed source-proof identities",
                                ));
                            }
                            staged_proof_completed = Some(*canonical_discovery_hash);
                        }
                        CanonicalRunRecordPayload::Discovery {
                            kind: DiscoveryEventKind::SnapshotPublished,
                            canonical_discovery_hash,
                            ..
                        } => {
                            if mechanism_request.is_some() {
                                synchronize_mechanism_target_knowledge(
                                    &exact,
                                    mechanism.as_mut(),
                                    false,
                                )?;
                                let bytes = store
                                    .read_blob(
                                        MECHANISM_OBSERVABLE_CHECKPOINT_BLOB_KIND_V1,
                                        &canonical_discovery_hash.to_lowercase_hex(),
                                    )
                                    .map_err(|error| {
                                        ExactStreamCoordinatorError::context(
                                            "cannot verify published mechanism checkpoint blob",
                                            error,
                                        )
                                    })?;
                                require_canonical_mechanism_checkpoint_bytes(
                                    &stream,
                                    &exact,
                                    mechanism.as_ref(),
                                    derive_probe_progress(
                                        staged_manifest_blob,
                                        staged_manifest,
                                        staged_proof_set,
                                        staged_proof_completed,
                                        &candidate_ranks,
                                    )?
                                    .complete(),
                                    false,
                                    &bytes,
                                )?;
                            } else {
                                let bytes = store
                                    .read_blob(
                                        EXACT_OBSERVABLE_SNAPSHOT_BLOB_KIND_V1,
                                        &canonical_discovery_hash.to_lowercase_hex(),
                                    )
                                    .map_err(|error| {
                                        ExactStreamCoordinatorError::context(
                                            "cannot verify published snapshot blob",
                                            error,
                                        )
                                    })?;
                                require_canonical_snapshot_bytes(
                                    &stream,
                                    &exact,
                                    query,
                                    report_request,
                                    derive_probe_progress(
                                        staged_manifest_blob,
                                        staged_manifest,
                                        staged_proof_set,
                                        staged_proof_completed,
                                        &candidate_ranks,
                                    )?,
                                    &bytes,
                                )?;
                            }
                        }
                        CanonicalRunRecordPayload::Discovery {
                            kind: DiscoveryEventKind::SnapshotUnavailablePublished,
                            canonical_discovery_hash,
                            ..
                        } => {
                            if mechanism_request.is_some() {
                                synchronize_mechanism_target_knowledge(
                                    &exact,
                                    mechanism.as_mut(),
                                    false,
                                )?;
                                let bytes = store
                                    .read_blob(
                                        MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_BLOB_KIND_V1,
                                        &canonical_discovery_hash.to_lowercase_hex(),
                                    )
                                    .map_err(|error| {
                                        ExactStreamCoordinatorError::context(
                                            "cannot verify published mechanism checkpoint-unavailable blob",
                                            error,
                                        )
                                    })?;
                                require_canonical_mechanism_checkpoint_bytes(
                                    &stream,
                                    &exact,
                                    mechanism.as_ref(),
                                    derive_probe_progress(
                                        staged_manifest_blob,
                                        staged_manifest,
                                        staged_proof_set,
                                        staged_proof_completed,
                                        &candidate_ranks,
                                    )?
                                    .complete(),
                                    true,
                                    &bytes,
                                )?;
                            } else {
                                let bytes = store
                                    .read_blob(
                                        EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_BLOB_KIND_V1,
                                        &canonical_discovery_hash.to_lowercase_hex(),
                                    )
                                    .map_err(|error| {
                                        ExactStreamCoordinatorError::context(
                                            "cannot verify published snapshot-unavailable receipt blob",
                                            error,
                                        )
                                    })?;
                                require_canonical_snapshot_unavailable_bytes(
                                    &stream,
                                    derive_probe_progress(
                                        staged_manifest_blob,
                                        staged_manifest,
                                        staged_proof_set,
                                        staged_proof_completed,
                                        &candidate_ranks,
                                    )?
                                    .complete(),
                                    exact.closed_case_count(),
                                    &bytes,
                                )?;
                            }
                        }
                        CanonicalRunRecordPayload::Discovery {
                            kind: DiscoveryEventKind::TerminalResultPublished,
                            canonical_discovery_hash,
                            ..
                        } => {
                            let bytes = store
                                .read_blob(
                                    EXACT_SEMANTIC_ANSWER_BLOB_KIND_V1,
                                    &canonical_discovery_hash.to_lowercase_hex(),
                                )
                                .map_err(|error| {
                                    ExactStreamCoordinatorError::context(
                                        "cannot verify published terminal-result blob",
                                        error,
                                    )
                                })?;
                            require_canonical_terminal_result_bytes(
                                &stream,
                                &exact,
                                query,
                                report_request,
                                &bytes,
                            )?;
                            if content_digest(&bytes) != *canonical_discovery_hash {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "terminal-result blob bytes disagree with their journaled raw digest",
                                ));
                            }
                            let receipt = ExactTerminalPublicationReceiptV1 {
                                blob_digest: *canonical_discovery_hash,
                                payload_hash: TerminalPayloadHash::from_canonical_semantic_payload(
                                    &bytes,
                                ),
                            };
                            if staged_terminal_result.is_some_and(|previous| previous != receipt) {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "unchanged semantic evidence has conflicting terminal-result publications",
                                ));
                            }
                            staged_terminal_result = Some(receipt);
                        }
                        CanonicalRunRecordPayload::CoveragePlanAccepted { plan, .. } => {
                            if staged_proof_set.is_some() || staged_proof_completed.is_some() {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "source-proof coverage is repeated or outside the prepared probe phase",
                                ));
                            }
                            let manifest = staged_manifest.ok_or_else(|| {
                                ExactStreamCoordinatorError::invalid(
                                    "source-proof coverage precedes its durable prepared manifest",
                                )
                            })?;
                            require_coverage_matches_probe_manifest(plan, manifest)?;
                            staged_proof_set = Some(plan.proof_set_id());
                            staged_terminal_result = None;
                        }
                        CanonicalRunRecordPayload::FrontierTransition {
                            producer_kind: FrontierEvidenceKind::ProbeCandidateBatchClassification,
                            newly_closed,
                            ..
                        } => {
                            let manifest = staged_manifest.ok_or_else(|| {
                                ExactStreamCoordinatorError::invalid(
                                    "source-probe candidate evidence precedes its manifest",
                                )
                            })?;
                            if staged_proof_set != Some(manifest.proof_set_id())
                                || staged_proof_completed.is_some()
                                || !newly_closed.open_obligations().is_empty()
                            {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "source-probe candidate evidence is outside its active probe phase",
                                ));
                            }
                            for interval in newly_closed.open_cases().intervals() {
                                let mut rank = interval.start();
                                while rank < interval.end_exclusive() {
                                    if !candidate_ranks.contains(&rank) {
                                        return Err(ExactStreamCoordinatorError::invalid(
                                            "source-probe candidate evidence closes an undiscovered rank",
                                        ));
                                    }
                                    rank = rank.checked_add(1).ok_or_else(|| {
                                        ExactStreamCoordinatorError::invalid(
                                            "source-probe candidate rank overflow",
                                        )
                                    })?;
                                }
                            }
                            staged_terminal_result = None;
                        }
                        CanonicalRunRecordPayload::FrontierTransition { .. } => {
                            if staged_proof_completed.is_none() {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "non-probe frontier evidence precedes the source-probe milestone",
                                ));
                            }
                            staged_terminal_result = None;
                        }
                        CanonicalRunRecordPayload::SemanticObservation { .. } => {
                            if staged_proof_completed.is_none() {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "non-probe semantic observation precedes the source-probe milestone",
                                ));
                            }
                            staged_terminal_result = None;
                        }
                        CanonicalRunRecordPayload::TerminalSeal {
                            terminal_payload_hash,
                            ..
                        } => {
                            if staged_terminal_result.map(|receipt| receipt.payload_hash())
                                != Some(*terminal_payload_hash)
                            {
                                return Err(ExactStreamCoordinatorError::invalid(
                                    "terminal seal does not match the latest published semantic answer",
                                ));
                            }
                        }
                        _ => {}
                    }

                    stream.replay_committed(payload, &event).map_err(|error| {
                        ExactStreamCoordinatorError::context("stream reducer replay failed", error)
                    })?;
                    if let Some(next) = staged_candidates {
                        candidate_ranks = next;
                    }
                    candidate_ranks
                        .retain(|rank| stream.frontier().open_cases().contains_rank(*rank));
                    source_proof_set_id = staged_proof_set;
                    source_probe_manifest_blob = staged_manifest_blob;
                    source_probe_manifest = staged_manifest;
                    source_proof_completed = staged_proof_completed;
                    published_terminal_result = staged_terminal_result;
                    previous_record_serviced_snapshot_view = services_snapshot_view;
                }
                if source_proof_completed.is_some() && source_proof_set_id.is_none() {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "source-proof completion marker precedes an accepted coverage plan",
                    ));
                }
                if source_proof_completed.is_some() && source_proof_completed != source_proof_set_id
                {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "source-proof completion marker disagrees with accepted coverage",
                    ));
                }
                if source_proof_set_id.is_some() && source_probe_manifest.is_none() {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "source-proof coverage has no durable prepared manifest",
                    ));
                }
                verify_reducer_frontier(&exact, &stream)?;
                candidate_ranks.retain(|rank| stream.frontier().open_cases().contains_rank(*rank));
                derive_probe_progress(
                    source_probe_manifest_blob,
                    source_probe_manifest,
                    source_proof_set_id,
                    source_proof_completed,
                    &candidate_ranks,
                )?;

                // Debt survives Resumed/Recovered and any crash before the
                // corresponding full-snapshot or snapshot-unavailable record.
                // Only a successfully replayed observer publication clears it;
                // a sealed stream has no continuation to service.
                let pending_observable_snapshot_on_resume =
                    stream.lifecycle() != RunLifecycle::Sealed && observable_snapshot_debt;
                let mut coordinator = Self {
                    store,
                    stream,
                    exact,
                    mechanism,
                    mechanism_request,
                    evaluator: None,
                    statements,
                    source_dir,
                    artifacts,
                    accepted_query_index,
                    query,
                    report_request,
                    replay_closure: expected.replay_closure,
                    writer_fence: None,
                    active_lease: None,
                    candidate_ranks,
                    source_probe_manifest_blob,
                    source_probe_manifest,
                    source_proof_set_id,
                    source_proof_completed,
                    published_terminal_result,
                    pending_observable_snapshot_on_resume,
                    last_committed_record_serviced_snapshot_view:
                        previous_record_serviced_snapshot_view,
                };
                if coordinator.stream.lifecycle() != RunLifecycle::Sealed {
                    coordinator.acquire_continuation_lease()?;
                }
                Ok(coordinator)
            }
        }
    }

    pub(super) fn stream(&self) -> &ExploreRunStream {
        &self.stream
    }

    pub(super) const fn report_request(&self) -> ExploreReportRequest {
        self.report_request
    }

    pub(super) const fn mechanism_checkpoint_enabled(&self) -> bool {
        self.mechanism_request.is_some()
    }

    fn require_exact_only_publication_contract(
        &self,
        action: &str,
    ) -> Result<(), ExactStreamCoordinatorError> {
        if self.mechanism_request.is_some() {
            return Err(ExactStreamCoordinatorError::invalid(format!(
                "cannot {action}: mechanism-enabled streams remain private until the observable snapshot and terminal-result schemas include mechanism closure"
            )));
        }
        Ok(())
    }

    /// The next confirmed matching CaseId which has not crossed a durable
    /// mechanism observation boundary. This projection allocates no rank
    /// list or support difference.
    pub(super) fn next_mechanism_rank_hint(
        &mut self,
    ) -> Result<Option<u128>, ExactStreamCoordinatorError> {
        synchronize_mechanism_target_knowledge(&self.exact, self.mechanism.as_mut(), false)?;
        self.mechanism
            .as_ref()
            .ok_or_else(|| {
                ExactStreamCoordinatorError::invalid(
                    "this Explore stream identity does not authorize mechanism replay",
                )
            })?
            .first_known_unprocessed_rank()
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot select the next confirmed mechanism target",
                    error,
                )
            })
    }

    /// Commit one already fresh-replay-confirmed mechanism block through the
    /// same blob -> journal -> reducer ordering as exact case evidence. This
    /// private seam is intentionally not called by the current CLI: runtime
    /// instrumentation must mint the validated token first.
    pub(super) fn commit_validated_mechanism_observation_batch(
        &mut self,
        validated: ValidatedMechanismObservationBatchV1,
    ) -> Result<usize, ExactStreamCoordinatorError> {
        if self.source_proof_completed.is_none() {
            return Err(ExactStreamCoordinatorError::invalid(
                "mechanism observation cannot precede the completed source-probe milestone",
            ));
        }
        let request = self.mechanism_request.clone().ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "this Explore stream identity does not authorize mechanism evidence",
            )
        })?;
        synchronize_mechanism_target_knowledge(&self.exact, self.mechanism.as_mut(), false)?;

        let bytes = encode_mechanism_observation_batch_v1(&request, validated.proposal()).map_err(
            |error| {
                ExactStreamCoordinatorError::context(
                    "cannot encode mechanism observation batch",
                    error,
                )
            },
        )?;
        let blob_digest = content_digest(&bytes);
        let facts = mechanism_evidence_projection(
            self.stream.header().case_universe(),
            &request,
            &validated,
        )?;
        let prepared_mechanism = self
            .mechanism
            .as_ref()
            .ok_or_else(|| {
                ExactStreamCoordinatorError::invalid(
                    "mechanism request exists without its evidence reducer",
                )
            })?
            .prepare_observation_batch(validated)
            .map_err(|error| {
                if error.is_reducer_capacity() {
                    ExactStreamCoordinatorError::mechanism_capacity(
                        "mechanism observation reached durable reducer capacity",
                        error,
                    )
                } else {
                    ExactStreamCoordinatorError::context(
                        "mechanism observation conflicts with reducer state",
                        error,
                    )
                }
            })?;
        let lease = self.require_active_lease()?;
        let transition = self
            .stream
            .prepare_observation(
                lease,
                super::run_stream::ObservationEvidenceKind::MechanismObserved,
                facts,
                blob_digest,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot prepare mechanism semantic observation",
                    error,
                )
            })?;
        self.store
            .install_blob(
                MECHANISM_OBSERVATION_BLOB_KIND_V1,
                &blob_digest.to_lowercase_hex(),
                &bytes,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot install mechanism observation blob",
                    error,
                )
            })?;
        self.commit_prepared(transition)?;
        self.mechanism
            .as_mut()
            .expect("prepared mechanism reducer must still exist")
            .apply_prepared_observation_batch(prepared_mechanism);
        Ok(bytes.len())
    }

    /// Current mechanism materialization at this cursor. Capacity pressure is
    /// returned as typed unavailability by the mechanism reducer rather than
    /// publishing a truncated incidence graph.
    pub(super) fn mechanism_snapshot(
        &mut self,
    ) -> Result<Option<MechanismObservedEvidence>, ExactStreamCoordinatorError> {
        synchronize_mechanism_target_knowledge(&self.exact, self.mechanism.as_mut(), true)?;
        self.mechanism
            .as_ref()
            .map(|mechanism| {
                mechanism.snapshot().map_err(|error| {
                    if error.is_snapshot_capacity() {
                        ExactStreamCoordinatorError::snapshot_capacity(
                            "cannot materialize mechanism snapshot",
                            error,
                        )
                    } else {
                        ExactStreamCoordinatorError::context(
                            "cannot materialize mechanism snapshot",
                            error,
                        )
                    }
                })
            })
            .transpose()
    }

    pub(super) fn prepare_mechanism_checkpoint_publication(
        &mut self,
        authority: &mut ExactStreamSnapshotPublicationAuthority,
    ) -> Result<PreparedMechanismObservableCheckpointPublicationV1, ExactStreamCoordinatorError>
    {
        if !authority.consume_preparation() {
            return Err(ExactStreamCoordinatorError::invalid(
                "snapshot-publication authority has already prepared one materialized view",
            ));
        }
        self.prepare_mechanism_checkpoint_publication_inner()
    }

    #[cfg(test)]
    pub(super) fn prepare_mechanism_checkpoint_publication_for_test(
        &mut self,
    ) -> Result<PreparedMechanismObservableCheckpointPublicationV1, ExactStreamCoordinatorError>
    {
        self.prepare_mechanism_checkpoint_publication_inner()
    }

    fn prepare_mechanism_checkpoint_publication_inner(
        &mut self,
    ) -> Result<PreparedMechanismObservableCheckpointPublicationV1, ExactStreamCoordinatorError>
    {
        if self.mechanism_request.is_none() {
            return Err(ExactStreamCoordinatorError::invalid(
                "exact-only stream identity does not authorize a mechanism checkpoint",
            ));
        }
        if !self.probe_phase_complete() {
            return Err(ExactStreamCoordinatorError::invalid(
                "mechanism checkpoint publication requires the completed source-probe milestone",
            ));
        }
        synchronize_mechanism_target_knowledge(&self.exact, self.mechanism.as_mut(), false)?;
        let authoritative_target = self.exact.authoritative_admissible_match_support();
        let summary = self
            .mechanism
            .as_ref()
            .ok_or_else(|| {
                ExactStreamCoordinatorError::invalid(
                    "mechanism-enabled stream has no mechanism evidence reducer",
                )
            })?
            .checkpoint_summary_with_authoritative_target(authoritative_target.as_ref())
            .map_err(|error| {
                if error.is_snapshot_capacity() {
                    ExactStreamCoordinatorError::snapshot_capacity(
                        "cannot derive mechanism checkpoint summary",
                        error,
                    )
                } else {
                    ExactStreamCoordinatorError::context(
                        "cannot derive mechanism checkpoint summary",
                        error,
                    )
                }
            })?;
        let metadata = MechanismObservableCheckpointMetadataV1::from_running_cursor(
            self.stream.header().identity().schemas().snapshot(),
            self.stream.cursor(),
            true,
            self.stream.header().case_universe().case_count(),
            self.exact.closed_case_count(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot derive mechanism checkpoint metadata",
                error,
            )
        })?;
        match render_mechanism_observable_checkpoint_json_line_v1(&metadata, &summary) {
            Ok(canonical_json_line) => Ok(PreparedMechanismObservableCheckpointPublicationV1 {
                cursor: self.stream.cursor(),
                canonical_json_line,
                kind: PreparedMechanismObservableCheckpointPublicationKindV1::Included,
            }),
            Err(error) if error.is_capacity_limit() => {
                let canonical_json_line =
                    render_mechanism_observable_checkpoint_unavailable_json_line_v1(
                        &metadata, &summary,
                    )
                    .map_err(|unavailable_error| {
                        ExactStreamCoordinatorError::context(
                            "cannot render bounded mechanism checkpoint-unavailable receipt",
                            unavailable_error,
                        )
                    })?;
                Ok(PreparedMechanismObservableCheckpointPublicationV1 {
                    cursor: self.stream.cursor(),
                    canonical_json_line,
                    kind:
                        PreparedMechanismObservableCheckpointPublicationKindV1::CapacityUnavailable,
                })
            }
            Err(error) => Err(ExactStreamCoordinatorError::context(
                "cannot render mechanism checkpoint",
                error,
            )),
        }
    }

    /// A preceding journal-only pause is durable but has no observer view.
    /// Service that debt before this resumed invocation advances the semantic
    /// frontier, otherwise repeated time-boxed slices could defer forever.
    pub(super) const fn pending_observable_snapshot_on_resume(&self) -> bool {
        self.pending_observable_snapshot_on_resume
    }

    /// Materialize the exact current-evidence case view requested by this
    /// stream. The result is total over the declared universe: every rank not
    /// yet present in one typed closed support resolves to an explicit open
    /// terminal. Capacity failures return a status value, never a graph
    /// prefix.
    pub(super) fn prepare_case_graph_publication(
        &self,
    ) -> Result<PreparedExactCaseGraphPublication, ExactStreamCoordinatorError> {
        Ok(PreparedExactCaseGraphPublication {
            publication: prepare_case_graph_publication(
                &self.stream,
                &self.exact,
                self.report_request,
            )?,
            run_id: self.stream.header().run_id(),
            report_request: self.report_request,
            closed_case_count: self.exact.closed_case_count(),
            classification_support_identity_hashes: self
                .exact
                .classification_support_identity_hashes(),
        })
    }

    fn require_prepared_case_graph_publication(
        &self,
        prepared: &PreparedExactCaseGraphPublication,
    ) -> Result<(), ExactStreamCoordinatorError> {
        if prepared.run_id != self.stream.header().run_id()
            || prepared.report_request != self.report_request
            || prepared.closed_case_count != self.exact.closed_case_count()
            || prepared.classification_support_identity_hashes
                != self.exact.classification_support_identity_hashes()
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "prepared case-graph publication is stale or belongs to another report request",
            ));
        }
        require_case_graph_request_matches(self.report_request, prepared.publication())
    }

    pub(super) fn prepare_observable_snapshot_publication(
        &self,
        authority: &mut ExactStreamSnapshotPublicationAuthority,
    ) -> Result<PreparedExactObservableSnapshotPublication, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("prepare observable snapshot")?;
        if !authority.consume_preparation() {
            return Err(ExactStreamCoordinatorError::invalid(
                "snapshot-publication authority has already prepared one materialized view",
            ));
        }
        self.prepare_observable_snapshot_publication_inner()
    }

    #[cfg(test)]
    pub(super) fn prepare_observable_snapshot_publication_for_test(
        &self,
    ) -> Result<PreparedExactObservableSnapshotPublication, ExactStreamCoordinatorError> {
        self.prepare_observable_snapshot_publication_inner()
    }

    #[cfg(test)]
    pub(super) fn prepare_observable_snapshot_unavailable_for_test(
        &self,
        detail: impl Into<String>,
    ) -> Result<PreparedExactObservableSnapshotPublication, ExactStreamCoordinatorError> {
        let probe_progress = self.probe_progress()?;
        self.prepare_observable_snapshot_capacity_status_inner(
            probe_progress.complete(),
            self.exact.closed_case_count(),
            detail.into(),
        )
    }

    fn prepare_observable_snapshot_publication_inner(
        &self,
    ) -> Result<PreparedExactObservableSnapshotPublication, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("prepare observable snapshot")?;
        let probe_progress = self.probe_progress()?;
        let snapshot = self.exact.observable_snapshot();
        let metadata = match ExactObservableSnapshotMetadataV1::from_checked_stream(
            &self.stream,
            self.query,
            probe_progress,
        ) {
            Ok(metadata) => metadata,
            Err(error) if error.is_capacity_limit() => {
                return self.prepare_observable_snapshot_capacity_status_inner(
                    probe_progress.complete(),
                    snapshot.closed_case_count,
                    format!("cannot derive canonical observable snapshot metadata: {error}"),
                )
            }
            Err(error) => {
                return Err(ExactStreamCoordinatorError::context(
                    "cannot derive canonical observable snapshot metadata",
                    error,
                ))
            }
        };
        let case_graph = match self.prepare_case_graph_publication() {
            Ok(case_graph) => case_graph,
            Err(error) if error.is_snapshot_publication_capacity() => {
                return self.prepare_observable_snapshot_capacity_status_inner(
                    probe_progress.complete(),
                    snapshot.closed_case_count,
                    error.to_string(),
                )
            }
            Err(error) => return Err(error),
        };
        let canonical_json_line = match render_exact_observable_snapshot_json_line_v1(
            &metadata,
            &snapshot,
            case_graph.publication(),
        ) {
            Ok(bytes) => bytes,
            Err(error) if error.is_capacity_limit() => {
                return self.prepare_observable_snapshot_capacity_status_inner(
                    probe_progress.complete(),
                    snapshot.closed_case_count,
                    format!("cannot render canonical observable snapshot: {error}"),
                )
            }
            Err(error) => {
                return Err(ExactStreamCoordinatorError::context(
                    "cannot render canonical observable snapshot",
                    error,
                ))
            }
        };
        Ok(PreparedExactObservableSnapshotPublication {
            cursor: self.stream.cursor(),
            canonical_json_line,
            probe_milestone_complete: probe_progress.complete(),
            closed_case_count: snapshot.closed_case_count,
            kind: PreparedExactObservableSnapshotPublicationKind::Included,
        })
    }

    fn prepare_observable_snapshot_capacity_status_inner(
        &self,
        probe_milestone_complete: bool,
        closed_case_count: u128,
        detail: String,
    ) -> Result<PreparedExactObservableSnapshotPublication, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("prepare snapshot-unavailable receipt")?;
        let canonical_json_line = render_exact_observable_snapshot_unavailable_json_line_v1(
            &self.stream,
            probe_milestone_complete,
            closed_case_count,
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot render bounded observable snapshot-unavailable receipt",
                error,
            )
        })?;
        Ok(PreparedExactObservableSnapshotPublication {
            cursor: self.stream.cursor(),
            canonical_json_line,
            probe_milestone_complete,
            closed_case_count,
            kind: PreparedExactObservableSnapshotPublicationKind::CapacityUnavailable {
                detail: detail.into_boxed_str(),
            },
        })
    }

    pub(super) fn exact_snapshot(&self) -> ExactEvidenceSnapshotV1 {
        self.exact.observable_snapshot()
    }

    pub(super) fn closed_case_count(&self) -> u128 {
        self.exact.closed_case_count()
    }

    pub(super) fn replay_closure(&self) -> RequiredObligationId {
        self.replay_closure
    }

    /// Freshly evaluate the complete selected representative/extrema witness
    /// set, install its canonical manifest and close only the report-wide
    /// replay obligation.  Failure or an operationally open witness commits
    /// nothing, so the manifest remains an atomic finalization unit. The
    /// manifest codec's fixed witness/byte caps bound this v1 phase; timed
    /// exploration slices do not call it implicitly.
    pub(super) fn close_replay_obligation(
        &mut self,
    ) -> Result<ExactReplayClosureAdvance, ExactStreamCoordinatorError> {
        if !self.probe_phase_complete() {
            return Err(ExactStreamCoordinatorError::invalid(
                "source-probe phase must commit before representative/extrema replay begins",
            ));
        }
        if !self
            .stream
            .frontier()
            .open_obligations()
            .contains(&self.replay_closure)
        {
            return Ok(ExactReplayClosureAdvance::AlreadyClosed);
        }
        if !self.stream.frontier().open_cases().is_empty() {
            return Err(ExactStreamCoordinatorError::invalid(
                "representative/extrema replay cannot begin before case classification closes",
            ));
        }

        let witness_ranks = match exact_replay_witness_ranks_v1(&self.exact) {
            Ok(ranks) => ranks,
            Err(error) => {
                return Ok(ExactReplayClosureAdvance::LimitReached {
                    detail: error.to_string(),
                })
            }
        };
        let witness_count = witness_ranks.len();
        let manifest = if witness_ranks.is_empty() {
            ExactReplayClosureManifestV1::new(
                Vec::<ExactCaseObservationProposalV1>::new().into_boxed_slice(),
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot construct empty exact replay manifest",
                    error,
                )
            })?
        } else {
            // Witness replay is a fresh semantic pass, not a continuation of
            // the mutable interpreter used for canonical classification.
            self.evaluator = None;
            let mut confirmed = Vec::with_capacity(witness_ranks.len());
            let mut retained_encoded_bytes = 8_usize + 4;
            for rank in witness_ranks.iter().copied() {
                match self.evaluate_rank(rank, "fresh exact witness replay failed")? {
                    ExactStreamCaseAttempt::Complete(observation) => {
                        if observation.rank() != rank {
                            return Err(ExactStreamCoordinatorError::invalid(format!(
                                "exact witness evaluator returned rank {} while replaying rank {rank}",
                                observation.rank()
                            )));
                        }
                        let observation_bytes =
                            observation.canonical_encoded_len().map_err(|error| {
                                ExactStreamCoordinatorError::invalid(format!(
                                    "cannot size exact replay observation at rank {rank}: {error}"
                                ))
                            })?;
                        let Some(next_retained_bytes) = retained_encoded_bytes
                            .checked_add(4)
                            .and_then(|bytes| bytes.checked_add(observation_bytes))
                        else {
                            return Ok(ExactReplayClosureAdvance::LimitReached {
                                detail: "atomic replay-manifest size overflow".to_string(),
                            });
                        };
                        if next_retained_bytes > EXACT_STREAM_ATOMIC_REPLAY_ACCUMULATION_BUDGET_V1 {
                            return Ok(ExactReplayClosureAdvance::LimitReached {
                                detail: format!(
                                    "selected replay witnesses exceed the {}-byte atomic accumulation budget",
                                    EXACT_STREAM_ATOMIC_REPLAY_ACCUMULATION_BUDGET_V1
                                ),
                            });
                        }
                        retained_encoded_bytes = next_retained_bytes;
                        confirmed.push(observation);
                    }
                    ExactStreamCaseAttempt::Open(reason) => {
                        return Ok(ExactReplayClosureAdvance::WitnessOpen { rank, reason })
                    }
                }
            }
            let (proposal, _) =
                seal_local_evaluator_observation_batch_v1(confirmed).map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot seal freshly replayed exact witnesses",
                        error,
                    )
                })?;
            ExactReplayClosureManifestV1::new(proposal.observations).map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot construct exact replay manifest",
                    error,
                )
            })?
        };
        let validation =
            validate_exact_replay_closure_v1(&self.exact, &manifest).map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "fresh exact replay manifest does not close selected witnesses",
                    error,
                )
            })?;
        let normalized_witness_digest = validation.normalized_witness_digest();
        let bytes = encode_exact_replay_closure_manifest_v1(&manifest).map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot encode exact replay-closure manifest",
                error,
            )
        })?;
        let blob_digest = content_digest(&bytes);
        let newly_closed = RequiredFrontier::new(
            ExactCaseSupport::empty(self.stream.header().case_universe()),
            [self.replay_closure],
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot construct replay-obligation closure frontier",
                error,
            )
        })?;
        let fact = SemanticEvidenceFact::new(
            SemanticEvidenceLayer::RepresentativeSelection,
            normalized_witness_digest,
            SemanticEvidenceSubject::obligations([self.replay_closure]).map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot construct replay-obligation semantic subject",
                    error,
                )
            })?,
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot construct replay-obligation semantic fact",
                error,
            )
        })?;
        let lease = self.require_active_lease()?;
        let transition = self
            .stream
            .prepare_frontier_transition(
                lease,
                FrontierEvidenceKind::RepresentativeSelectionClosed,
                newly_closed,
                [fact],
                blob_digest,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot prepare replay-obligation frontier transition",
                    error,
                )
            })?;
        self.store
            .install_blob(
                EXACT_REPLAY_CLOSURE_BLOB_KIND_V1,
                &blob_digest.to_lowercase_hex(),
                &bytes,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot install exact replay-closure manifest",
                    error,
                )
            })?;
        self.commit_prepared(transition)?;
        verify_reducer_frontier(&self.exact, &self.stream)?;
        Ok(ExactReplayClosureAdvance::Closed {
            witness_count,
            normalized_witness_digest,
        })
    }

    /// Deterministic rank which the next one-CaseId advancement will request.
    /// The outer resource driver binds its generation-scoped dispatch permit
    /// to this exact rank before calling `advance_one_case`.
    pub(super) fn next_open_rank_hint(&mut self) -> Option<u128> {
        self.next_open_rank()
    }

    pub(super) fn probe_phase_complete(&self) -> bool {
        self.source_proof_completed.is_some()
    }

    pub(super) fn probe_progress(
        &self,
    ) -> Result<ExactSourceProbeProgressV1, ExactStreamCoordinatorError> {
        derive_probe_progress(
            self.source_probe_manifest_blob,
            self.source_probe_manifest,
            self.source_proof_set_id,
            self.source_proof_completed,
            &self.candidate_ranks,
        )
    }

    pub(super) fn probe_phase(
        &self,
    ) -> Result<ExactSourceProbePhaseV1, ExactStreamCoordinatorError> {
        Ok(self.probe_progress()?.phase())
    }

    pub(super) fn next_probe_candidate_rank_hint(&self) -> Option<u128> {
        if self.source_proof_set_id.is_none() || self.source_proof_completed.is_some() {
            return None;
        }
        let open_cases = self.stream.frontier().open_cases();
        self.candidate_ranks
            .iter()
            .copied()
            .find(|rank| open_cases.contains_rank(*rank))
    }

    /// Durably select canonical CaseId traversal when optional source-proof
    /// planning reported a failure for which
    /// `SourceProofPlanError::permits_canonical_fallback()` is true.  The
    /// caller remains responsible for that error classification: extraction,
    /// certification and accounting failures must never enter this path.
    ///
    /// Persist the canonical-fallback transcript without yet accepting
    /// coverage or claiming the probe milestone complete.
    pub(super) fn persist_probe_fallback_manifest(
        &mut self,
    ) -> Result<ExactSourceProbeProgressV1, ExactStreamCoordinatorError> {
        if self.source_probe_manifest.is_some() {
            return self.probe_progress();
        }
        if self.source_proof_set_id.is_some() || self.source_proof_completed.is_some() {
            return Err(ExactStreamCoordinatorError::invalid(
                "source-probe fallback cannot be prepared after coverage or completion",
            ));
        }
        let candidate_bytes = encode_source_proof_candidate_ranks_v1(
            &[],
            self.stream.header().case_universe().case_count(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot encode canonical-fallback candidate set",
                error,
            )
        })?;
        let candidate_blob = content_digest(&candidate_bytes);
        let proof_set_id =
            canonical_probe_fallback_proof_set_id(self.stream.header(), candidate_blob);
        let manifest = ExactSourceProbeManifestV1::canonical_fallback(
            proof_set_id,
            candidate_blob,
            self.stream.header().case_universe().case_count(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot prepare canonical source-probe fallback manifest",
                error,
            )
        })?;
        self.store
            .install_blob(
                SOURCE_CANDIDATE_BLOB_V1,
                &candidate_blob.to_lowercase_hex(),
                &candidate_bytes,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot install canonical-fallback candidate blob",
                    error,
                )
            })?;
        self.persist_probe_manifest(manifest)?;
        self.candidate_ranks.clear();
        self.probe_progress()
    }

    /// Run the bounded checked source adapter once, then durably publish its
    /// compact transcript before coverage or candidate evaluation begins.
    pub(super) fn persist_source_probe_manifest(
        &mut self,
        source_plan: &SourceProofPlan,
    ) -> Result<ExactSourceProbeProgressV1, ExactStreamCoordinatorError> {
        if self.source_probe_manifest.is_some() {
            return self.probe_progress();
        }
        if self.source_proof_set_id.is_some() || self.source_proof_completed.is_some() {
            return Err(ExactStreamCoordinatorError::invalid(
                "source-probe manifest cannot be prepared after coverage or completion",
            ));
        }
        let prepared =
            prepare_source_proof_exact_coverage_v1(self.query, source_plan).map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot prepare exact source-proof contribution",
                    error,
                )
            })?;
        let candidate_bytes = prepared.encode_candidate_ranks_v1().map_err(|error| {
            ExactStreamCoordinatorError::context("cannot encode source-proof candidates", error)
        })?;
        let candidate_blob = content_digest(&candidate_bytes);
        let (closed_regions, candidate_case_ids, summary, output_digest, producer) =
            prepared.into_parts();
        let proof_set_id = CanonicalDigest::from_sha256_bytes(output_digest.bytes());
        require_source_proof_identity(self.stream.header(), producer)?;
        let region_blob = match closed_regions {
            None => None,
            Some(validated) => {
                let projection = region_evidence_projection(self.stream.header(), &validated)?;
                let expected_closed = summary
                    .sealed_proof_nonmatch_cases()
                    .checked_add(summary.sealed_structural_excluded_cases())
                    .ok_or_else(|| {
                        ExactStreamCoordinatorError::invalid(
                            "source-proof coverage summary exceeds u128::MAX",
                        )
                    })?;
                if projection.support.case_count() != expected_closed {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "source-proof region support disagrees with its coverage summary",
                    ));
                }
                let proposal = proposal_from_validated_regions(&validated)?;
                let bytes = encode_exact_closed_region_batch_v1(&proposal).map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot encode exact source-proof region blob",
                        error,
                    )
                })?;
                let digest = content_digest(&bytes);
                self.store
                    .install_blob(EXACT_REGION_BLOB_V1, &digest.to_lowercase_hex(), &bytes)
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot install exact source-proof region blob",
                            error,
                        )
                    })?;
                Some(digest)
            }
        };
        self.store
            .install_blob(
                SOURCE_CANDIDATE_BLOB_V1,
                &candidate_blob.to_lowercase_hex(),
                &candidate_bytes,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot install source-proof candidate blob",
                    error,
                )
            })?;
        let manifest = ExactSourceProbeManifestV1::checked_source(
            proof_set_id,
            candidate_blob,
            candidate_case_ids.len(),
            region_blob,
            summary,
            source_plan.total_outer_profiles(),
            source_plan.analyzed_outer_profiles(),
            source_plan.proof_incomplete_profiles(),
            source_plan.profile_limit_reached(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot construct checked source-probe manifest",
                error,
            )
        })?;
        self.persist_probe_manifest(manifest)?;
        self.candidate_ranks = candidate_case_ids
            .iter()
            .map(|case_id| case_id.rank)
            .collect();
        self.probe_progress()
    }

    /// Accept coverage from the already persisted manifest. Every input is
    /// restored from authenticated blobs, so a restart never reruns source
    /// analysis after `ProbePlanPrepared` is durable.
    pub(super) fn accept_prepared_probe_coverage(
        &mut self,
        shard_width: NonZeroU64,
    ) -> Result<ExactSourceProbeProgressV1, ExactStreamCoordinatorError> {
        if self.source_proof_set_id.is_some() {
            return self.probe_progress();
        }
        let manifest = self.source_probe_manifest.ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "source-probe coverage cannot be accepted before its manifest",
            )
        })?;
        if self.source_proof_completed.is_some() {
            return Err(ExactStreamCoordinatorError::invalid(
                "source-probe coverage cannot be accepted after completion",
            ));
        }

        let (certified_closed, semantic_facts, prepared_exact) = match manifest.closed_region_blob()
        {
            None => (
                ExactCaseSupport::empty(self.stream.header().case_universe()),
                Vec::new(),
                None,
            ),
            Some(region_blob) => {
                let bytes = self
                    .store
                    .read_blob(EXACT_REGION_BLOB_V1, &region_blob.to_lowercase_hex())
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot read prepared source-probe region blob",
                            error,
                        )
                    })?;
                let proposal = decode_exact_closed_region_batch_v1(&bytes).map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot decode prepared source-probe region blob",
                        error,
                    )
                })?;
                let validated =
                    restore_coordinator_committed_region_batch_v1(proposal, |validated| {
                        let projection =
                            region_evidence_projection(self.stream.header(), validated)
                                .map_err(|error| error.to_string())?;
                        if projection.support.case_count()
                            != manifest.coverage().certified_closed_case_count()
                        {
                            return Err(
                                "prepared source-probe regions disagree with manifest coverage"
                                    .to_string(),
                            );
                        }
                        Ok(())
                    })
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot restore prepared source-probe regions",
                            error,
                        )
                    })?;
                let projection = region_evidence_projection(self.stream.header(), &validated)?;
                let prepared_exact =
                    self.exact
                        .prepare_closed_region_batch(validated)
                        .map_err(|error| {
                            ExactStreamCoordinatorError::context(
                                "prepared source-probe regions conflict with exact evidence",
                                error,
                            )
                        })?;
                (projection.support, projection.facts, Some(prepared_exact))
            }
        };
        let residual_open = self
            .stream
            .frontier()
            .open_cases()
            .subtract_exact(&certified_closed)
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "prepared source-probe coverage does not fit the open frontier",
                    error,
                )
            })?;
        if certified_closed.case_count() != manifest.coverage().certified_closed_case_count()
            || residual_open.case_count() != manifest.coverage().residual_open_case_count()
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "prepared source-probe coverage counts disagree with the manifest",
            ));
        }
        if self
            .candidate_ranks
            .iter()
            .any(|rank| !residual_open.contains_rank(*rank))
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "prepared source-probe candidate lies outside residual coverage",
            ));
        }
        let proof_receipt_hash = manifest
            .closed_region_blob()
            .unwrap_or_else(|| manifest.candidate_blob());
        let coverage = CoveragePlan::new(
            self.stream.header(),
            manifest.proof_set_id(),
            certified_closed,
            residual_open,
            semantic_facts,
            proof_receipt_hash,
            NonZeroU64::new(1).expect("one is nonzero"),
            shard_width,
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot prepare persisted source-probe coverage plan",
                error,
            )
        })?;
        require_coverage_matches_probe_manifest(&coverage, manifest)?;
        let lease = self.require_active_lease()?;
        let transition = self
            .stream
            .prepare_coverage_plan(lease, coverage)
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot prepare persisted source-probe coverage transition",
                    error,
                )
            })?;
        self.commit_prepared(transition)?;
        if let Some(prepared_exact) = prepared_exact {
            self.exact
                .apply_prepared_closed_region_batch(prepared_exact);
        }
        self.source_proof_set_id = Some(manifest.proof_set_id());
        verify_reducer_frontier(&self.exact, &self.stream)?;
        self.probe_progress()
    }

    pub(super) fn complete_prepared_probe(
        &mut self,
    ) -> Result<ExactSourceProbeProgressV1, ExactStreamCoordinatorError> {
        if self.source_proof_completed.is_some() {
            return self.probe_progress();
        }
        let manifest = self.source_probe_manifest.ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "source probe cannot complete before its manifest is prepared",
            )
        })?;
        if self.source_proof_set_id != Some(manifest.proof_set_id()) {
            return Err(ExactStreamCoordinatorError::invalid(
                "source probe cannot complete before matching coverage is accepted",
            ));
        }
        let open_cases = self.stream.frontier().open_cases();
        self.candidate_ranks
            .retain(|rank| open_cases.contains_rank(*rank));
        if !self.candidate_ranks.is_empty() {
            return Err(ExactStreamCoordinatorError::invalid(
                "source probe cannot complete while discovered candidates remain open",
            ));
        }
        self.commit_discovery(
            DiscoveryEventKind::ProbePlanCompleted,
            manifest.proof_set_id(),
        )?;
        self.source_proof_completed = Some(manifest.proof_set_id());
        self.probe_progress()
    }

    /// Evaluate one bounded block drawn exclusively from still-open ranks in
    /// the durable source-probe manifest. Residual CaseIds are deliberately
    /// invisible until `ProbePlanCompleted` has been committed.
    pub(super) fn advance_bounded_probe_candidate_batch(
        &mut self,
        case_cap: NonZeroU16,
    ) -> Result<ExactProbeCandidateBatchAdvance, ExactStreamCoordinatorError> {
        if case_cap.get() > EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP {
            return Err(ExactStreamCoordinatorError::invalid(format!(
                "source-probe candidate batch cap {} exceeds first-generation limit {}",
                case_cap.get(),
                EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP,
            )));
        }
        if self.source_probe_manifest.is_none()
            || self.source_proof_set_id.is_none()
            || self.source_proof_completed.is_some()
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "source-probe candidates require prepared manifest, accepted coverage and incomplete milestone",
            ));
        }

        let mut confirmed = Vec::with_capacity(usize::from(case_cap.get()));
        let mut selected_ranks = BTreeSet::<u128>::new();
        let mut singleton_encoded_bytes = 0_usize;
        let stop = loop {
            if confirmed.len() == usize::from(case_cap.get()) {
                break match self.next_probe_candidate_excluding(&selected_ranks) {
                    Some(next_rank) => ExactProbeCandidateBatchStop::CaseCapReached { next_rank },
                    None => ExactProbeCandidateBatchStop::CandidatesComplete,
                };
            }

            let Some(rank) = self.next_probe_candidate_excluding(&selected_ranks) else {
                if confirmed.is_empty() {
                    return Ok(ExactProbeCandidateBatchAdvance::CandidatesComplete);
                }
                break ExactProbeCandidateBatchStop::CandidatesComplete;
            };
            let observation = match self
                .evaluate_rank(rank, "bounded source-probe candidate evaluation failed")?
            {
                ExactStreamCaseAttempt::Complete(observation) => observation,
                ExactStreamCaseAttempt::Open(reason) => {
                    if confirmed.is_empty() {
                        return Ok(ExactProbeCandidateBatchAdvance::CaseOpen { rank, reason });
                    }
                    break ExactProbeCandidateBatchStop::CaseOpen { rank, reason };
                }
            };
            if observation.rank() != rank {
                return Err(ExactStreamCoordinatorError::invalid(format!(
                    "exact evaluator returned rank {} while coordinating source-probe rank {rank}",
                    observation.rank(),
                )));
            }
            let observation_bytes = observation.canonical_encoded_len().map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot measure canonical source-probe observation",
                    error,
                )
            })?;
            let next_encoded_bytes = singleton_encoded_bytes
                .checked_add(observation_bytes)
                .ok_or_else(|| {
                    ExactStreamCoordinatorError::invalid(
                        "source-probe observation batch byte accounting overflow",
                    )
                })?;
            if !confirmed.is_empty()
                && next_encoded_bytes > EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1
            {
                break ExactProbeCandidateBatchStop::ByteTargetReached { next_rank: rank };
            }
            if !selected_ranks.insert(rank) {
                return Err(ExactStreamCoordinatorError::invalid(format!(
                    "bounded source-probe batch selected CaseId rank {rank} more than once",
                )));
            }
            confirmed.push(observation);
            singleton_encoded_bytes = next_encoded_bytes;
            if singleton_encoded_bytes >= EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1 {
                break match self.next_probe_candidate_excluding(&selected_ranks) {
                    Some(next_rank) => {
                        ExactProbeCandidateBatchStop::ByteTargetReached { next_rank }
                    }
                    None => ExactProbeCandidateBatchStop::CandidatesComplete,
                };
            }
        };

        let (proposal, validated) =
            seal_local_evaluator_observation_batch_v1(confirmed).map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot seal bounded source-probe observation batch",
                    error,
                )
            })?;
        let ranks = proposal
            .observations
            .iter()
            .map(|observation| observation.case_id.rank)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let canonical_blob_bytes = self.commit_observation_batch(
            proposal,
            validated,
            FrontierEvidenceKind::ProbeCandidateBatchClassification,
        )?;
        for rank in ranks.iter().copied() {
            self.candidate_ranks.remove(&rank);
        }
        Ok(ExactProbeCandidateBatchAdvance::Committed {
            ranks,
            canonical_blob_bytes,
            closed_case_count: self.exact.closed_case_count(),
            stop,
        })
    }

    /// Evaluate and durably commit at most one whole CaseId.  Candidate hints
    /// are preferred, then the first canonical open rank is used.  No parallel
    /// work or speculative batch remains in memory across this boundary.
    pub(super) fn advance_one_case(
        &mut self,
    ) -> Result<ExactStreamAdvance, ExactStreamCoordinatorError> {
        if !self.probe_phase_complete() {
            return Err(ExactStreamCoordinatorError::invalid(
                "checked source probes must complete before residual CaseId evaluation begins",
            ));
        }
        let Some(rank) = self.next_open_rank() else {
            // Terminal witness replay is a separately invoked, atomically
            // bounded finalization phase. A deadline-driven outer slice must
            // stop here instead of entering that manifest loop implicitly.
            return Ok(ExactStreamAdvance::ClassificationClosedFinalizationPending);
        };
        let confirmed = match self.evaluate_rank(rank, "exact CaseId evaluation failed")? {
            ExactStreamCaseAttempt::Complete(confirmed) => confirmed,
            ExactStreamCaseAttempt::Open(reason) => {
                return Ok(ExactStreamAdvance::CaseOpen { rank, reason })
            }
        };
        if confirmed.rank() != rank {
            return Err(ExactStreamCoordinatorError::invalid(format!(
                "exact evaluator returned rank {} while coordinating rank {rank}",
                confirmed.rank()
            )));
        }
        let (proposal, validated) = seal_local_evaluator_observation_batch_v1(vec![confirmed])
            .map_err(|error| {
                ExactStreamCoordinatorError::context("cannot seal local exact observation", error)
            })?;
        self.commit_observation_batch(
            proposal,
            validated,
            FrontierEvidenceKind::SingletonClassification,
        )?;
        self.candidate_ranks.remove(&rank);
        Ok(ExactStreamAdvance::Committed {
            rank,
            closed_case_count: self.exact.closed_case_count(),
        })
    }

    /// Evaluate a deterministic candidate-first block of whole CaseIds and
    /// commit its completed prefix as one durable observation batch.
    ///
    /// The caller cap is additionally bounded by the first-generation
    /// coordinator limit. Canonical singleton lengths enforce a conservative
    /// byte target before sealing; an over-target lookahead is discarded and
    /// therefore remains open. An individually encodable first observation
    /// may exceed the soft target, but is never combined with another case.
    pub(super) fn advance_bounded_case_batch(
        &mut self,
        case_cap: NonZeroU16,
    ) -> Result<ExactStreamBatchAdvance, ExactStreamCoordinatorError> {
        if case_cap.get() > EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP {
            return Err(ExactStreamCoordinatorError::invalid(format!(
                "exact stream CaseId batch cap {} exceeds first-generation limit {}",
                case_cap.get(),
                EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP,
            )));
        }
        if !self.probe_phase_complete() {
            return Err(ExactStreamCoordinatorError::invalid(
                "checked source probes must complete before residual CaseId evaluation begins",
            ));
        }

        let mut confirmed = Vec::with_capacity(usize::from(case_cap.get()));
        let mut selected_ranks = BTreeSet::<u128>::new();
        let mut singleton_encoded_bytes = 0_usize;
        let stop = loop {
            if confirmed.len() == usize::from(case_cap.get()) {
                break match self.next_open_rank_excluding(&selected_ranks) {
                    Some(next_rank) => ExactStreamBatchStop::CaseCapReached { next_rank },
                    None => ExactStreamBatchStop::ClassificationClosedFinalizationPending,
                };
            }

            let Some(rank) = self.next_open_rank_excluding(&selected_ranks) else {
                if confirmed.is_empty() {
                    return Ok(ExactStreamBatchAdvance::ClassificationClosedFinalizationPending);
                }
                break ExactStreamBatchStop::ClassificationClosedFinalizationPending;
            };
            let observation =
                match self.evaluate_rank(rank, "bounded exact CaseId batch evaluation failed")? {
                    ExactStreamCaseAttempt::Complete(observation) => observation,
                    ExactStreamCaseAttempt::Open(reason) => {
                        if confirmed.is_empty() {
                            return Ok(ExactStreamBatchAdvance::CaseOpen { rank, reason });
                        }
                        break ExactStreamBatchStop::CaseOpen { rank, reason };
                    }
                };
            if observation.rank() != rank {
                return Err(ExactStreamCoordinatorError::invalid(format!(
                    "exact evaluator returned rank {} while coordinating batch rank {rank}",
                    observation.rank(),
                )));
            }
            let observation_bytes = observation.canonical_encoded_len().map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot measure canonical exact observation",
                    error,
                )
            })?;
            let next_encoded_bytes = singleton_encoded_bytes
                .checked_add(observation_bytes)
                .ok_or_else(|| {
                    ExactStreamCoordinatorError::invalid(
                        "exact observation batch byte accounting overflow",
                    )
                })?;
            if !confirmed.is_empty()
                && next_encoded_bytes > EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1
            {
                // Evaluation is side-effect-free with respect to both durable
                // reducers. Dropping this trusted token leaves the whole
                // CaseId open for an exact retry in the next block.
                break ExactStreamBatchStop::ByteTargetReached { next_rank: rank };
            }
            if !selected_ranks.insert(rank) {
                return Err(ExactStreamCoordinatorError::invalid(format!(
                    "bounded exact batch selected CaseId rank {rank} more than once",
                )));
            }
            confirmed.push(observation);
            singleton_encoded_bytes = next_encoded_bytes;

            if singleton_encoded_bytes >= EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1 {
                break match self.next_open_rank_excluding(&selected_ranks) {
                    Some(next_rank) => ExactStreamBatchStop::ByteTargetReached { next_rank },
                    None => ExactStreamBatchStop::ClassificationClosedFinalizationPending,
                };
            }
        };

        let (proposal, validated) =
            seal_local_evaluator_observation_batch_v1(confirmed).map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot seal bounded local exact observation batch",
                    error,
                )
            })?;
        let ranks = proposal
            .observations
            .iter()
            .map(|observation| observation.case_id.rank)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let producer_kind = if ranks.len() == 1 {
            FrontierEvidenceKind::SingletonClassification
        } else {
            FrontierEvidenceKind::BoundedExactBatchClassification
        };
        let canonical_blob_bytes =
            self.commit_observation_batch(proposal, validated, producer_kind)?;
        for rank in ranks.iter().copied() {
            self.candidate_ranks.remove(&rank);
        }
        Ok(ExactStreamBatchAdvance::Committed {
            ranks,
            canonical_blob_bytes,
            closed_case_count: self.exact.closed_case_count(),
            stop,
        })
    }

    pub(super) fn pause(
        &mut self,
        reason: PauseReason,
    ) -> Result<ExploreRunCursor, ExactStreamCoordinatorError> {
        let lease = self.require_active_lease()?;
        let transition = self.stream.prepare_pause(lease, reason).map_err(|error| {
            ExactStreamCoordinatorError::context("cannot prepare pause transition", error)
        })?;
        self.commit_prepared(transition)?;
        self.active_lease = None;
        self.writer_fence = None;
        Ok(self.stream.cursor())
    }

    /// Acquire a fresh fence and append `Resumed`.  This is useful when a live
    /// caller intentionally sliced and paused without dropping the lock.
    pub(super) fn resume(&mut self) -> Result<ExploreRunCursor, ExactStreamCoordinatorError> {
        if self.stream.lifecycle() != RunLifecycle::Paused {
            return Err(ExactStreamCoordinatorError::invalid(
                "only a paused Explore stream can be resumed",
            ));
        }
        self.acquire_continuation_lease()?;
        Ok(self.stream.cursor())
    }

    /// Install one coordinator-minted canonical projection and commit only its
    /// content pointer as operational journal provenance. The snapshot does
    /// not alter the normalized evidence root. Its cursor must still be the
    /// current pre-publication cursor, so the snapshot hash never needs to
    /// commit the pointer event which will name that hash.
    pub(super) fn publish_prepared_snapshot(
        &mut self,
        prepared: &PreparedExactObservableSnapshotPublication,
    ) -> Result<CanonicalDigest, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("publish observable snapshot")?;
        if prepared.cursor() != self.stream.cursor() {
            return Err(ExactStreamCoordinatorError::invalid(
                "prepared observable snapshot belongs to a stale stream cursor",
            ));
        }
        let (blob_kind, event_kind) = match &prepared.kind {
            PreparedExactObservableSnapshotPublicationKind::Included => (
                EXACT_OBSERVABLE_SNAPSHOT_BLOB_KIND_V1,
                DiscoveryEventKind::SnapshotPublished,
            ),
            PreparedExactObservableSnapshotPublicationKind::CapacityUnavailable { .. } => (
                EXACT_OBSERVABLE_SNAPSHOT_UNAVAILABLE_BLOB_KIND_V1,
                DiscoveryEventKind::SnapshotUnavailablePublished,
            ),
        };
        self.install_discovery_blob(blob_kind, prepared.canonical_json_line(), event_kind)
    }

    pub(super) fn publish_prepared_mechanism_checkpoint(
        &mut self,
        prepared: &PreparedMechanismObservableCheckpointPublicationV1,
    ) -> Result<CanonicalDigest, ExactStreamCoordinatorError> {
        if self.mechanism_request.is_none() {
            return Err(ExactStreamCoordinatorError::invalid(
                "exact-only stream identity does not authorize mechanism checkpoint publication",
            ));
        }
        if prepared.cursor() != self.stream.cursor() {
            return Err(ExactStreamCoordinatorError::invalid(
                "prepared mechanism checkpoint belongs to a stale stream cursor",
            ));
        }
        let (blob_kind, event_kind) = match prepared.kind {
            PreparedMechanismObservableCheckpointPublicationKindV1::Included => (
                MECHANISM_OBSERVABLE_CHECKPOINT_BLOB_KIND_V1,
                DiscoveryEventKind::SnapshotPublished,
            ),
            PreparedMechanismObservableCheckpointPublicationKindV1::CapacityUnavailable => (
                MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_BLOB_KIND_V1,
                DiscoveryEventKind::SnapshotUnavailablePublished,
            ),
        };
        self.install_discovery_blob(blob_kind, prepared.canonical_json_line(), event_kind)
    }

    pub(super) const fn published_terminal_result(
        &self,
    ) -> Option<ExactTerminalPublicationReceiptV1> {
        self.published_terminal_result
    }

    /// Render and publish the complete history-independent semantic answer.
    ///
    /// This deliberately uses the reducer's full snapshot. Observable pause
    /// snapshots may expose a bounded prefix, but a terminal answer is never
    /// silently truncated. The current single-blob v1 renderer has a 64 MiB
    /// hard ceiling; chunked terminal publication is a later protocol.
    pub(super) fn publish_current_terminal_result(
        &mut self,
        case_graph_publication: &PreparedExactCaseGraphPublication,
    ) -> Result<ExactTerminalPublicationAdvanceV1, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("publish terminal result")?;
        self.require_prepared_case_graph_publication(case_graph_publication)?;
        let publication = case_graph_publication.publication();
        if let ExactPreparedCaseGraphPublicationV1::CapacityLimited {
            resource,
            maximum,
            required_at_least,
        } = publication
        {
            return Ok(ExactTerminalPublicationAdvanceV1::LimitReached {
                phase: "case_graph_publication",
                detail: format!(
                    "requested complete case graph requires at least {required_at_least} {}, exceeding the fixed maximum {maximum}",
                    resource.name()
                ),
            });
        }
        let metadata = ExactSemanticAnswerMetadataV1::from_checked_stream(&self.stream, self.query)
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot derive canonical terminal-result metadata",
                    error,
                )
            })?;
        let bytes = match render_exact_semantic_answer_json_v1(
            &metadata,
            &self.exact.snapshot(),
            publication,
        ) {
            Ok(bytes) => bytes,
            Err(error) if error.is_capacity_limit() => {
                return Ok(ExactTerminalPublicationAdvanceV1::LimitReached {
                    phase: "terminal_publication",
                    detail: error.to_string(),
                })
            }
            Err(error) => {
                return Err(ExactStreamCoordinatorError::context(
                    "cannot render canonical terminal result",
                    error,
                ))
            }
        };
        self.install_prepared_terminal_result_bytes(&bytes)
            .map(ExactTerminalPublicationAdvanceV1::Published)
    }

    /// Install bytes minted by `publish_current_terminal_result`. This helper
    /// is private so arbitrary caller-supplied JSON cannot cross the journal
    /// publication seam.
    fn install_prepared_terminal_result_bytes(
        &mut self,
        canonical_bytes: &[u8],
    ) -> Result<ExactTerminalPublicationReceiptV1, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("install terminal result")?;
        let receipt = ExactTerminalPublicationReceiptV1 {
            blob_digest: content_digest(canonical_bytes),
            payload_hash: TerminalPayloadHash::from_canonical_semantic_payload(canonical_bytes),
        };
        if let Some(previous) = self.published_terminal_result {
            if previous == receipt {
                return Ok(previous);
            }
            return Err(ExactStreamCoordinatorError::invalid(
                "current semantic evidence already has a different terminal-result publication",
            ));
        }
        let digest = self.install_discovery_blob(
            EXACT_SEMANTIC_ANSWER_BLOB_KIND_V1,
            canonical_bytes,
            DiscoveryEventKind::TerminalResultPublished,
        )?;
        if digest != receipt.blob_digest() {
            return Err(ExactStreamCoordinatorError::invalid(
                "terminal-result store digest disagrees with its prepared publication receipt",
            ));
        }
        self.published_terminal_result = Some(receipt);
        Ok(receipt)
    }

    /// Consume the latest authenticated publication and seal exact exhaustion.
    /// The fixed versioned method commitment prevents callers from describing
    /// the same terminal bytes with an ad-hoc completion method.
    pub(super) fn seal_completed_exact_exhaustion(
        &mut self,
        receipt: ExactTerminalPublicationReceiptV1,
    ) -> Result<ExploreRunCursor, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("seal completed terminal result")?;
        if self.published_terminal_result != Some(receipt) {
            return Err(ExactStreamCoordinatorError::invalid(
                "terminal seal receipt does not match the latest published semantic answer",
            ));
        }
        let lease = self.require_active_lease()?;
        let transition = self
            .stream
            .prepare_seal(
                lease,
                TerminalSealKind::Completed,
                receipt.payload_hash(),
                TerminalMethodHash::from_canonical_method(
                    EXACT_STREAM_COMPLETED_EXACT_EXHAUSTION_METHOD_V1,
                ),
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context("cannot prepare terminal seal", error)
            })?;
        self.commit_prepared(transition)?;
        self.active_lease = None;
        self.writer_fence = None;
        Ok(self.stream.cursor())
    }

    /// Read the immutable terminal artifact named by a sealed run and verify
    /// every link from raw blob address through canonical answer bytes to the
    /// terminal semantic commitment.
    pub(super) fn read_verified_terminal_result_bytes(
        &self,
    ) -> Result<Vec<u8>, ExactStreamCoordinatorError> {
        self.require_exact_only_publication_contract("read exact-only terminal result")?;
        if self.stream.lifecycle() != RunLifecycle::Sealed {
            return Err(ExactStreamCoordinatorError::invalid(
                "terminal-result readback requires a sealed Explore stream",
            ));
        }
        let seal = self.stream.terminal_seal().ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "sealed Explore stream is missing its terminal commitment",
            )
        })?;
        if seal.kind() == TerminalSealKind::Completed
            && seal.method_hash()
                != TerminalMethodHash::from_canonical_method(
                    EXACT_STREAM_COMPLETED_EXACT_EXHAUSTION_METHOD_V1,
                )
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "completed terminal seal has an unexpected finalization method commitment",
            ));
        }
        let receipt = self.published_terminal_result.ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "sealed Explore stream is missing its terminal-result publication receipt",
            )
        })?;
        if seal.terminal_payload_hash() != receipt.payload_hash() {
            return Err(ExactStreamCoordinatorError::invalid(
                "sealed terminal payload hash disagrees with its publication receipt",
            ));
        }
        let bytes = self
            .store
            .read_blob(
                EXACT_SEMANTIC_ANSWER_BLOB_KIND_V1,
                &receipt.blob_digest().to_lowercase_hex(),
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot read sealed terminal-result blob",
                    error,
                )
            })?;
        if content_digest(&bytes) != receipt.blob_digest()
            || TerminalPayloadHash::from_canonical_semantic_payload(&bytes)
                != receipt.payload_hash()
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "sealed terminal-result bytes disagree with their raw or semantic commitment",
            ));
        }
        // Fresh publication validates these canonical bytes before installing
        // the receipt. Recovery validates them again while replaying the
        // `TerminalResultPublished` record. Readback therefore needs only the
        // raw-address and sealed semantic commitments; rerendering the entire
        // answer here would duplicate a potentially large admitted operation.
        Ok(bytes)
    }

    fn acquire_continuation_lease(&mut self) -> Result<(), ExactStreamCoordinatorError> {
        let lifecycle = self.stream.lifecycle();
        if lifecycle == RunLifecycle::Sealed {
            return Err(ExactStreamCoordinatorError::invalid(
                "a sealed Explore stream cannot acquire another writer fence",
            ));
        }
        let generation = self.store.next_writer_fence_generation().ok_or_else(|| {
            ExactStreamCoordinatorError::invalid("writer-fence generation space is exhausted")
        })?;
        let writer_id = ExploreWriterId::new(os_random_nonzero_digest("writer id")?);
        let identity =
            canonical_writer_fence_identity(self.stream.header().run_id(), generation, writer_id);
        let receipt = self
            .store
            .acquire_writer_fence(&identity)
            .map_err(|error| {
                ExactStreamCoordinatorError::context("cannot acquire durable writer fence", error)
            })?;
        let receipt_hash =
            CanonicalDigest::from_lowercase_sha256("writer_fence_receipt", receipt.receipt_hash())
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "writer-fence receipt is not canonical",
                        error,
                    )
                })?;
        let lease = FencedWriterLease::new(
            self.stream.header().run_id(),
            generation,
            writer_id,
            receipt_hash,
        );
        require_receipt_matches_lease(&receipt, lease, &identity)?;
        let cursor = self.stream.cursor();
        let transition = match lifecycle {
            RunLifecycle::Paused => self.stream.prepare_resume(cursor, lease),
            RunLifecycle::Running => self.stream.prepare_recovery(cursor, lease),
            RunLifecycle::Sealed => unreachable!("sealed lifecycle returned above"),
        }
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot prepare continuation lease transition",
                error,
            )
        })?;
        self.writer_fence = Some(receipt);
        self.active_lease = Some(lease);
        self.commit_prepared(transition)
    }

    fn commit_observation_batch(
        &mut self,
        proposal: ExactCaseObservationBatchProposalV1,
        validated: ValidatedExactCaseObservationBatchV1,
        producer_kind: FrontierEvidenceKind,
    ) -> Result<usize, ExactStreamCoordinatorError> {
        let bytes = encode_exact_case_observation_batch_v1(&proposal).map_err(|error| {
            ExactStreamCoordinatorError::context("cannot encode exact observation batch", error)
        })?;
        let blob_digest = content_digest(&bytes);
        let projection = observation_evidence_projection(self.stream.header(), &validated)?;
        match producer_kind {
            FrontierEvidenceKind::SingletonClassification
                if projection.support.case_count() != 1 =>
            {
                return Err(ExactStreamCoordinatorError::invalid(
                    "singleton classification transition must close exactly one CaseId",
                ));
            }
            FrontierEvidenceKind::BoundedExactBatchClassification
                if projection.support.case_count() < 2 =>
            {
                return Err(ExactStreamCoordinatorError::invalid(
                    "bounded exact batch transition must close at least two CaseIds",
                ));
            }
            FrontierEvidenceKind::BoundedExactBatchClassification
                if projection.support.case_count()
                    > u128::from(EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP) =>
            {
                return Err(ExactStreamCoordinatorError::invalid(
                    "bounded exact batch transition exceeds the first-generation CaseId cap",
                ));
            }
            FrontierEvidenceKind::BoundedExactBatchClassification
                if bytes.len() > EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1 =>
            {
                return Err(ExactStreamCoordinatorError::invalid(
                    "bounded exact batch transition exceeds its canonical byte target",
                ));
            }
            FrontierEvidenceKind::ProbeCandidateBatchClassification
                if projection.support.case_count() == 0 =>
            {
                return Err(ExactStreamCoordinatorError::invalid(
                    "source-probe candidate batch must close at least one CaseId",
                ));
            }
            FrontierEvidenceKind::ProbeCandidateBatchClassification
                if projection.support.case_count()
                    > u128::from(EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP) =>
            {
                return Err(ExactStreamCoordinatorError::invalid(
                    "source-probe candidate batch exceeds the first-generation CaseId cap",
                ));
            }
            FrontierEvidenceKind::ProbeCandidateBatchClassification
                if projection.support.case_count() > 1
                    && bytes.len() > EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1 =>
            {
                return Err(ExactStreamCoordinatorError::invalid(
                    "source-probe candidate batch exceeds its canonical byte target",
                ));
            }
            FrontierEvidenceKind::SingletonClassification
            | FrontierEvidenceKind::BoundedExactBatchClassification
            | FrontierEvidenceKind::ProbeCandidateBatchClassification
            | FrontierEvidenceKind::ExactExhaustion => {}
            FrontierEvidenceKind::CertifiedRegionClassification
            | FrontierEvidenceKind::RepresentativeSelectionClosed
            | FrontierEvidenceKind::MechanismTargetClosed => {
                return Err(ExactStreamCoordinatorError::invalid(
                    "exact observation batch has a non-observation producer kind",
                ));
            }
        }
        if producer_kind == FrontierEvidenceKind::ExactExhaustion
            && projection.support.case_count() != self.stream.frontier().open_cases().case_count()
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "exact-exhaustion observation batch leaves CaseIds open",
            ));
        }
        let newly_closed = RequiredFrontier::new(
            projection.support,
            std::iter::empty::<RequiredObligationId>(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot construct exact observation closure",
                error,
            )
        })?;
        let prepared_exact = self
            .exact
            .prepare_observation_batch(validated)
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "exact observation batch conflicts with reducer state",
                    error,
                )
            })?;
        let lease = self.require_active_lease()?;
        let transition = self
            .stream
            .prepare_frontier_transition(
                lease,
                producer_kind,
                newly_closed,
                projection.facts,
                blob_digest,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot prepare exact observation frontier transition",
                    error,
                )
            })?;
        self.store
            .install_blob(
                EXACT_OBSERVATION_BLOB_V1,
                &blob_digest.to_lowercase_hex(),
                &bytes,
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context("cannot install exact observation blob", error)
            })?;
        self.commit_prepared(transition)?;
        self.exact.apply_prepared_observation_batch(prepared_exact);
        verify_reducer_frontier(&self.exact, &self.stream)?;
        Ok(bytes.len())
    }

    fn install_discovery_blob(
        &mut self,
        kind: &str,
        bytes: &[u8],
        event_kind: DiscoveryEventKind,
    ) -> Result<CanonicalDigest, ExactStreamCoordinatorError> {
        let digest = content_digest(bytes);
        let lease = self.require_active_lease()?;
        let transition = self
            .stream
            .prepare_discovery(lease, event_kind, digest)
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot prepare content publication record",
                    error,
                )
            })?;
        self.store
            .install_blob(kind, &digest.to_lowercase_hex(), bytes)
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot install content-addressed publication",
                    error,
                )
            })?;
        self.commit_prepared(transition)?;
        Ok(digest)
    }

    fn persist_probe_manifest(
        &mut self,
        manifest: ExactSourceProbeManifestV1,
    ) -> Result<CanonicalDigest, ExactStreamCoordinatorError> {
        manifest
            .validate_for_header(self.stream.header())
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "source-probe manifest does not match the run header",
                    error,
                )
            })?;
        let bytes = encode_source_probe_manifest_v1(manifest).map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot encode canonical source-probe manifest",
                error,
            )
        })?;
        let digest = self.install_discovery_blob(
            SOURCE_PROBE_MANIFEST_BLOB_KIND_V1,
            &bytes,
            DiscoveryEventKind::ProbePlanPrepared,
        )?;
        self.source_probe_manifest_blob = Some(digest);
        self.source_probe_manifest = Some(manifest);
        Ok(digest)
    }

    fn commit_discovery(
        &mut self,
        kind: DiscoveryEventKind,
        digest: CanonicalDigest,
    ) -> Result<(), ExactStreamCoordinatorError> {
        let lease = self.require_active_lease()?;
        let transition = self
            .stream
            .prepare_discovery(lease, kind, digest)
            .map_err(|error| {
                ExactStreamCoordinatorError::context("cannot prepare discovery transition", error)
            })?;
        self.commit_prepared(transition)
    }

    fn commit_prepared(
        &mut self,
        prepared: PreparedRunTransition,
    ) -> Result<(), ExactStreamCoordinatorError> {
        let services_snapshot_view = matches!(
            prepared.payload(),
            CanonicalRunRecordPayload::Discovery {
                kind: DiscoveryEventKind::SnapshotPublished
                    | DiscoveryEventKind::SnapshotUnavailablePublished,
                ..
            }
        );
        let pauses = matches!(prepared.payload(), CanonicalRunRecordPayload::Paused { .. });
        let mechanism_checkpoint_not_yet_available =
            self.mechanism_request.is_some() && !self.probe_phase_complete();
        let pause_creates_snapshot_debt = pauses
            && !self.last_committed_record_serviced_snapshot_view
            && !mechanism_checkpoint_not_yet_available;
        let invalidates_terminal_payload = matches!(
            prepared.payload(),
            CanonicalRunRecordPayload::CoveragePlanAccepted { .. }
                | CanonicalRunRecordPayload::FrontierTransition { .. }
                | CanonicalRunRecordPayload::SemanticObservation { .. }
        );
        let bytes = encode_record(prepared.event(), prepared.payload()).map_err(|error| {
            ExactStreamCoordinatorError::context("cannot encode stream record", error)
        })?;
        let sequence = prepared.event().sequence();
        let journal_head = prepared.event().journal_head().to_lowercase_hex();
        let writer_fence = self.writer_fence.as_ref().ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "stream transition has no active durable writer-fence receipt",
            )
        })?;
        self.store
            .append_event(writer_fence, sequence, &journal_head, &bytes)
            .map_err(|error| {
                ExactStreamCoordinatorError::context("cannot append immutable stream record", error)
            })?;
        self.stream.apply_committed(prepared).map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot apply storage-authorized stream transition",
                error,
            )
        })?;
        if invalidates_terminal_payload {
            self.published_terminal_result = None;
        }
        if services_snapshot_view {
            self.pending_observable_snapshot_on_resume = false;
        } else if pauses {
            self.pending_observable_snapshot_on_resume = pause_creates_snapshot_debt;
        }
        self.last_committed_record_serviced_snapshot_view = services_snapshot_view;
        Ok(())
    }

    fn require_active_lease(&self) -> Result<FencedWriterLease, ExactStreamCoordinatorError> {
        self.active_lease.ok_or_else(|| {
            ExactStreamCoordinatorError::invalid("Explore stream has no active writer lease")
        })
    }

    fn ensure_evaluator_classified(
        &mut self,
    ) -> Result<&mut ExactStreamEvaluator<'a>, ExactEvaluatorEnsureError> {
        if self.evaluator.is_none() {
            self.evaluator = Some(
                ExactStreamEvaluator::prepare_classified(
                    self.statements,
                    self.source_dir,
                    self.artifacts,
                    self.accepted_query_index,
                    DEFAULT_EXPLORE_STEP_LIMIT,
                    DEFAULT_EXPLORE_COLLECTION_LIMIT,
                )
                .map_err(|error| match error {
                    ExactStreamEvaluatorPrepareError::OperationalLimit(stop) => {
                        ExactEvaluatorEnsureError::OperationalLimit(stop)
                    }
                    ExactStreamEvaluatorPrepareError::Failure(error) => {
                        ExactEvaluatorEnsureError::Failure(ExactStreamCoordinatorError::context(
                            "cannot lazily prepare exact stream evaluator",
                            error,
                        ))
                    }
                })?,
            );
        }
        self.evaluator.as_mut().ok_or_else(|| {
            ExactEvaluatorEnsureError::Failure(ExactStreamCoordinatorError::invalid(
                "exact stream evaluator initialization did not publish its instance",
            ))
        })
    }

    fn evaluate_rank(
        &mut self,
        rank: u128,
        context: &'static str,
    ) -> Result<ExactStreamCaseAttempt, ExactStreamCoordinatorError> {
        let evaluator = match self.ensure_evaluator_classified() {
            Ok(evaluator) => evaluator,
            Err(ExactEvaluatorEnsureError::OperationalLimit(stop)) => {
                return Ok(ExactStreamCaseAttempt::Open(stop));
            }
            Err(ExactEvaluatorEnsureError::Failure(error)) => return Err(error),
        };
        evaluator
            .evaluate_rank(rank)
            .map_err(|error| ExactStreamCoordinatorError::context(context, error))
    }

    fn next_open_rank(&mut self) -> Option<u128> {
        loop {
            let candidate = self.candidate_ranks.first().copied();
            match candidate {
                Some(rank) if self.stream.frontier().open_cases().contains_rank(rank) => {
                    return Some(rank)
                }
                Some(rank) => {
                    self.candidate_ranks.remove(&rank);
                }
                None => return self.stream.frontier().open_cases().first_rank(),
            }
        }
    }

    fn next_probe_candidate_excluding(&self, excluded: &BTreeSet<u128>) -> Option<u128> {
        let open_cases = self.stream.frontier().open_cases();
        self.candidate_ranks
            .iter()
            .copied()
            .find(|rank| open_cases.contains_rank(*rank) && !excluded.contains(rank))
    }

    /// Candidate-first scheduler projection over the still-committed
    /// frontier, excluding trusted observations staged for the current atomic
    /// block. The exclusion set is bounded by the 256-case coordinator cap.
    fn next_open_rank_excluding(&self, excluded: &BTreeSet<u128>) -> Option<u128> {
        let open_cases = self.stream.frontier().open_cases();
        if let Some(rank) = self
            .candidate_ranks
            .iter()
            .copied()
            .find(|rank| open_cases.contains_rank(*rank) && !excluded.contains(rank))
        {
            return Some(rank);
        }

        let mut rank = open_cases.first_rank();
        while let Some(current) = rank {
            if !excluded.contains(&current) {
                return Some(current);
            }
            let successor = current
                .checked_add(1)
                .expect("an in-universe half-open CaseId rank can advance once");
            rank = open_cases.first_rank_at_or_after(successor);
        }
        None
    }
}

fn derive_probe_progress(
    manifest_blob: Option<CanonicalDigest>,
    manifest: Option<ExactSourceProbeManifestV1>,
    coverage_proof_set: Option<CanonicalDigest>,
    completed_proof_set: Option<CanonicalDigest>,
    candidate_ranks: &BTreeSet<u128>,
) -> Result<ExactSourceProbeProgressV1, ExactStreamCoordinatorError> {
    if let Some(manifest) = manifest {
        if coverage_proof_set.is_some_and(|proof_set| proof_set != manifest.proof_set_id())
            || completed_proof_set.is_some_and(|proof_set| proof_set != manifest.proof_set_id())
        {
            return Err(ExactStreamCoordinatorError::invalid(
                "source-probe progress identities disagree with the prepared manifest",
            ));
        }
    }
    ExactSourceProbeProgressV1::derive(
        manifest_blob,
        manifest,
        coverage_proof_set.is_some(),
        completed_proof_set.is_some(),
        candidate_ranks.len(),
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context("cannot derive source-probe progress", error)
    })
}

fn read_source_probe_manifest(
    store: &ExploreRunStreamStore,
    header: &ExploreRunHeader,
    manifest_blob: CanonicalDigest,
) -> Result<(ExactSourceProbeManifestV1, BTreeSet<u128>), ExactStreamCoordinatorError> {
    let bytes = store
        .read_blob(
            SOURCE_PROBE_MANIFEST_BLOB_KIND_V1,
            &manifest_blob.to_lowercase_hex(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot read prepared source-probe manifest",
                error,
            )
        })?;
    let manifest = decode_source_probe_manifest_v1(&bytes).map_err(|error| {
        ExactStreamCoordinatorError::context("cannot decode prepared source-probe manifest", error)
    })?;
    manifest.validate_for_header(header).map_err(|error| {
        ExactStreamCoordinatorError::context(
            "prepared source-probe manifest does not match its run header",
            error,
        )
    })?;
    let candidate_bytes = store
        .read_blob(
            SOURCE_CANDIDATE_BLOB_V1,
            &manifest.candidate_blob().to_lowercase_hex(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot read prepared source-probe candidate blob",
                error,
            )
        })?;
    let candidate_ranks = decode_source_proof_candidate_ranks_v1(
        &candidate_bytes,
        header.case_universe().case_count(),
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot decode prepared source-probe candidates",
            error,
        )
    })?;
    if candidate_ranks.len() as u128 != manifest.candidate_count() {
        return Err(ExactStreamCoordinatorError::invalid(
            "prepared source-probe candidate blob count disagrees with its manifest",
        ));
    }
    if let Some(region_blob) = manifest.closed_region_blob() {
        store
            .read_blob(EXACT_REGION_BLOB_V1, &region_blob.to_lowercase_hex())
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot read prepared source-probe region blob",
                    error,
                )
            })?;
    }
    Ok((manifest, candidate_ranks.iter().copied().collect()))
}

fn require_coverage_matches_probe_manifest(
    plan: &CoveragePlan,
    manifest: ExactSourceProbeManifestV1,
) -> Result<(), ExactStreamCoordinatorError> {
    let expected_receipt = manifest
        .closed_region_blob()
        .unwrap_or_else(|| manifest.candidate_blob());
    if plan.proof_set_id() != manifest.proof_set_id()
        || plan.proof_receipt_hash() != expected_receipt
        || plan.certified_closed().case_count() != manifest.coverage().certified_closed_case_count()
        || plan.residual_open().case_count() != manifest.coverage().residual_open_case_count()
    {
        return Err(ExactStreamCoordinatorError::invalid(
            "accepted source-probe coverage disagrees with its prepared manifest",
        ));
    }
    Ok(())
}

fn require_canonical_snapshot_bytes(
    stream: &ExploreRunStream,
    exact: &ExactEvidenceReducer,
    query: &ExploreQueryIr,
    report_request: ExploreReportRequest,
    probe_progress: ExactSourceProbeProgressV1,
    bytes: &[u8],
) -> Result<(), ExactStreamCoordinatorError> {
    let metadata =
        ExactObservableSnapshotMetadataV1::from_checked_stream(stream, query, probe_progress)
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot reconstruct canonical observable snapshot metadata",
                    error,
                )
            })?;
    let case_graph = prepare_case_graph_publication(stream, exact, report_request)?;
    let expected = render_exact_observable_snapshot_json_line_v1(
        &metadata,
        &exact.observable_snapshot(),
        &case_graph,
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot reconstruct canonical observable snapshot",
            error,
        )
    })?;
    if expected.as_slice() != bytes {
        return Err(ExactStreamCoordinatorError::invalid(
            "observable snapshot blob does not encode its committed pre-publication cursor and exact evidence",
        ));
    }
    Ok(())
}

fn require_canonical_snapshot_unavailable_bytes(
    stream: &ExploreRunStream,
    probe_milestone_complete: bool,
    closed_case_count: u128,
    bytes: &[u8],
) -> Result<(), ExactStreamCoordinatorError> {
    let expected = render_exact_observable_snapshot_unavailable_json_line_v1(
        stream,
        probe_milestone_complete,
        closed_case_count,
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot reconstruct canonical observable snapshot-unavailable receipt",
            error,
        )
    })?;
    if expected.as_slice() != bytes {
        return Err(ExactStreamCoordinatorError::invalid(
            "snapshot-unavailable receipt blob does not encode its committed pre-publication cursor and exact progress",
        ));
    }
    Ok(())
}

fn require_canonical_mechanism_checkpoint_bytes(
    stream: &ExploreRunStream,
    exact: &ExactEvidenceReducer,
    mechanism: Option<&MechanismEvidenceReducerV1>,
    probe_milestone_complete: bool,
    unavailable: bool,
    bytes: &[u8],
) -> Result<(), ExactStreamCoordinatorError> {
    let mechanism = mechanism.ok_or_else(|| {
        ExactStreamCoordinatorError::invalid(
            "mechanism checkpoint journal record has no mechanism reducer",
        )
    })?;
    let authoritative_target = exact.authoritative_admissible_match_support();
    let summary = mechanism
        .checkpoint_summary_with_authoritative_target(authoritative_target.as_ref())
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot reconstruct mechanism checkpoint summary",
                error,
            )
        })?;
    let metadata = MechanismObservableCheckpointMetadataV1::from_running_cursor(
        stream.header().identity().schemas().snapshot(),
        stream.cursor(),
        probe_milestone_complete,
        stream.header().case_universe().case_count(),
        exact.closed_case_count(),
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot reconstruct mechanism checkpoint metadata",
            error,
        )
    })?;
    let expected = if unavailable {
        match render_mechanism_observable_checkpoint_json_line_v1(&metadata, &summary) {
            Err(error) if error.is_capacity_limit() => {
                render_mechanism_observable_checkpoint_unavailable_json_line_v1(
                    &metadata, &summary,
                )
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot reconstruct canonical mechanism checkpoint-unavailable receipt",
                        error,
                    )
                })?
            }
            Ok(_) => {
                return Err(ExactStreamCoordinatorError::invalid(
                    "mechanism checkpoint-unavailable receipt was published when the full checkpoint fit its fixed capacity",
                ))
            }
            Err(error) => {
                return Err(ExactStreamCoordinatorError::context(
                    "cannot reconstruct full mechanism checkpoint before unavailable receipt",
                    error,
                ))
            }
        }
    } else {
        render_mechanism_observable_checkpoint_json_line_v1(&metadata, &summary).map_err(
            |error| {
                ExactStreamCoordinatorError::context(
                    "cannot reconstruct canonical mechanism checkpoint",
                    error,
                )
            },
        )?
    };
    if expected.as_slice() != bytes {
        return Err(ExactStreamCoordinatorError::invalid(
            "mechanism checkpoint blob does not encode its committed pre-publication cursor and evidence",
        ));
    }
    Ok(())
}

fn require_canonical_terminal_result_bytes(
    stream: &ExploreRunStream,
    exact: &ExactEvidenceReducer,
    query: &ExploreQueryIr,
    report_request: ExploreReportRequest,
    bytes: &[u8],
) -> Result<(), ExactStreamCoordinatorError> {
    let metadata =
        ExactSemanticAnswerMetadataV1::from_checked_stream(stream, query).map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot reconstruct canonical semantic-answer metadata",
                error,
            )
        })?;
    let case_graph = prepare_case_graph_publication(stream, exact, report_request)?;
    let expected = render_exact_semantic_answer_json_v1(&metadata, &exact.snapshot(), &case_graph)
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot reconstruct canonical semantic answer",
                error,
            )
        })?;
    if expected.as_slice() != bytes {
        return Err(ExactStreamCoordinatorError::invalid(
            "terminal-result blob does not encode the committed exact semantic answer",
        ));
    }
    Ok(())
}

fn prepare_case_graph_publication(
    stream: &ExploreRunStream,
    exact: &ExactEvidenceReducer,
    request: ExploreReportRequest,
) -> Result<ExactPreparedCaseGraphPublicationV1, ExactStreamCoordinatorError> {
    if request.case_graph == ExploreCaseGraphRequest::Omit {
        return Ok(ExactPreparedCaseGraphPublicationV1::NotRequested);
    }

    let axis_cardinalities = stream.header().case_universe().axis_cardinalities();
    if axis_cardinalities.len() > DEFAULT_MAX_CASE_RANK_RUN_AXES {
        return Ok(ExactPreparedCaseGraphPublicationV1::CapacityLimited {
            resource: ExactCaseGraphPublicationResourceV1::LoweringAxes,
            maximum: DEFAULT_MAX_CASE_RANK_RUN_AXES,
            required_at_least: axis_cardinalities.len(),
        });
    }

    let Some(run_count) = exact.classification_rank_run_count() else {
        return Ok(ExactPreparedCaseGraphPublicationV1::CapacityLimited {
            resource: ExactCaseGraphPublicationResourceV1::LoweringRankRuns,
            maximum: DEFAULT_MAX_CASE_RANK_RUNS,
            required_at_least: DEFAULT_MAX_CASE_RANK_RUNS.saturating_add(1),
        });
    };
    if run_count > DEFAULT_MAX_CASE_RANK_RUNS {
        return Ok(ExactPreparedCaseGraphPublicationV1::CapacityLimited {
            resource: ExactCaseGraphPublicationResourceV1::LoweringRankRuns,
            maximum: DEFAULT_MAX_CASE_RANK_RUNS,
            required_at_least: run_count,
        });
    }

    let total_intervals = exact
        .classification_support_interval_count()
        .ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "case-classification support interval count exceeds usize::MAX",
            )
        })?;
    let supports = exact
        .classification_supports_bounded(total_intervals)
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot validate persistent case-classification supports",
                error,
            )
        })?
        .ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "bounded case-classification support traversal was unexpectedly refused",
            )
        })?;
    let runs = case_terminal_rank_runs(&supports, run_count)?;
    let axis_cardinalities = axis_cardinalities.to_vec();
    match lower_case_terminal_rank_runs(
        axis_cardinalities,
        runs,
        CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted),
    ) {
        Ok(graph) => Ok(ExactPreparedCaseGraphPublicationV1::Included(graph)),
        Err(CaseRankRunLoweringError::LimitExceeded {
            resource,
            observed,
            limit,
        }) => Ok(ExactPreparedCaseGraphPublicationV1::CapacityLimited {
            resource: case_graph_publication_resource(resource),
            maximum: limit,
            required_at_least: observed,
        }),
        Err(error @ CaseRankRunLoweringError::AllocationFailed { .. }) => {
            Err(ExactStreamCoordinatorError::snapshot_capacity(
                "cannot lower persistent case classifications into a total decision DAG",
                error,
            ))
        }
        Err(error) => Err(ExactStreamCoordinatorError::context(
            "cannot lower persistent case classifications into a total decision DAG",
            error,
        )),
    }
}

fn require_case_graph_request_matches(
    request: ExploreReportRequest,
    publication: &ExactPreparedCaseGraphPublicationV1,
) -> Result<(), ExactStreamCoordinatorError> {
    let matches = match (request.case_graph, publication) {
        (ExploreCaseGraphRequest::Omit, ExactPreparedCaseGraphPublicationV1::NotRequested) => true,
        (
            ExploreCaseGraphRequest::Include,
            ExactPreparedCaseGraphPublicationV1::Included(_)
            | ExactPreparedCaseGraphPublicationV1::CapacityLimited { .. },
        ) => true,
        _ => false,
    };
    if !matches {
        return Err(ExactStreamCoordinatorError::invalid(
            "prepared case-graph publication does not match the immutable report request",
        ));
    }
    Ok(())
}

fn case_terminal_rank_runs(
    supports: &ExactClosedClassificationSupportsV1,
    run_count: usize,
) -> Result<Vec<CaseTerminalRankRun>, ExactStreamCoordinatorError> {
    let mut runs = Vec::new();
    runs.try_reserve_exact(run_count).map_err(|_| {
        ExactStreamCoordinatorError::snapshot_capacity(
            "cannot allocate the bounded case-classification rank-run vector",
            "allocation request was refused",
        )
    })?;
    let mut fibers = [
        (
            supports
                .support(ExactClosedClassificationV1::Excluded)
                .iter_intervals()
                .peekable(),
            CaseTerminal::Excluded,
        ),
        (
            supports
                .support(ExactClosedClassificationV1::AdmissibleNonmatch)
                .iter_intervals()
                .peekable(),
            CaseTerminal::AdmissibleNonmatch,
        ),
        (
            supports
                .support(ExactClosedClassificationV1::AdmissibleMatch)
                .iter_intervals()
                .peekable(),
            CaseTerminal::AdmissibleMatch,
        ),
    ];
    loop {
        let mut next = None::<(u128, usize)>;
        for (index, (intervals, _)) in fibers.iter_mut().enumerate() {
            if let Some(interval) = intervals.peek() {
                let candidate = (interval.start(), index);
                if next.is_none_or(|current| candidate < current) {
                    next = Some(candidate);
                }
            }
        }
        let Some((_, index)) = next else {
            break;
        };
        let (intervals, terminal) = &mut fibers[index];
        let interval = intervals
            .next()
            .expect("peeked case-classification interval must still exist");
        runs.push(CaseTerminalRankRun::new(
            interval.start(),
            interval.end_exclusive(),
            terminal.clone(),
        ));
    }
    if runs.len() != run_count {
        return Err(ExactStreamCoordinatorError::invalid(format!(
            "materialized {} case-classification rank runs, expected {run_count}",
            runs.len()
        )));
    }
    Ok(runs)
}

fn case_graph_publication_resource(
    resource: CaseRankRunLoweringResource,
) -> ExactCaseGraphPublicationResourceV1 {
    match resource {
        CaseRankRunLoweringResource::Axes => ExactCaseGraphPublicationResourceV1::LoweringAxes,
        CaseRankRunLoweringResource::Runs => ExactCaseGraphPublicationResourceV1::LoweringRankRuns,
        CaseRankRunLoweringResource::Nodes => ExactCaseGraphPublicationResourceV1::LoweringNodes,
        CaseRankRunLoweringResource::Arcs => ExactCaseGraphPublicationResourceV1::LoweringArcs,
        CaseRankRunLoweringResource::OrdinalIntervals => {
            ExactCaseGraphPublicationResourceV1::LoweringOrdinalIntervals
        }
        CaseRankRunLoweringResource::AccountedBytes => {
            ExactCaseGraphPublicationResourceV1::LoweringAccountedBytes
        }
    }
}

fn validate_mechanism_request_for_checked_query(
    request: &CheckedMechanismObservationRequestV1,
    checked: &crate::CheckedExploreQueryView<'_>,
) -> Result<(), ExactStreamCoordinatorError> {
    request.validate().map_err(|error| {
        ExactStreamCoordinatorError::context("invalid checked mechanism request", error)
    })?;
    if request.observation.analysis_program.as_str()
        != checked.artifact.identity.analysis_program.as_str()
    {
        return Err(ExactStreamCoordinatorError::invalid(
            "mechanism request belongs to another checked analysis program",
        ));
    }
    let expected_query = MechanismQueryId::from_checked_query(checked).map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot derive mechanism identity from checked Explore query",
            error,
        )
    })?;
    if request.observation.query != expected_query {
        return Err(ExactStreamCoordinatorError::invalid(
            "mechanism request belongs to another checked Explore query or domain",
        ));
    }
    let expected_axes = checked
        .closed_query
        .universe
        .dimensions
        .iter()
        .map(|dimension| {
            dimension.domain.cardinality().exact().ok_or_else(|| {
                ExactStreamCoordinatorError::invalid(format!(
                    "Explore dimension `{}` cardinality exceeds u128::MAX",
                    dimension.name
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if request.observation.axis_cardinalities.as_ref() != expected_axes.as_slice() {
        return Err(ExactStreamCoordinatorError::invalid(
            "mechanism request belongs to another checked case universe",
        ));
    }
    Ok(())
}

fn exact_reducer_for_query(
    query: &ExploreQueryIr,
) -> Result<ExactEvidenceReducer, ExactStreamCoordinatorError> {
    let cardinalities = query
        .universe
        .dimensions
        .iter()
        .map(|dimension| {
            dimension.domain.cardinality().exact().ok_or_else(|| {
                ExactStreamCoordinatorError::invalid(format!(
                    "Explore dimension `{}` cardinality exceeds u128::MAX",
                    dimension.name
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shape = ExactProjectionShapeV1::new(
        query.query.output.key.len(),
        query.query.output.extrema.len(),
        query.query.output.show.len(),
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context("invalid exact projection shape", error)
    })?;
    let representative = match &query.query.output.representative {
        ExploreRepresentative::First { .. } => ExactRepresentativePolicyV1::First,
        ExploreRepresentative::Maximize { .. } => ExactRepresentativePolicyV1::Maximize,
        ExploreRepresentative::Minimize { .. } => ExactRepresentativePolicyV1::Minimize,
    };
    ExactEvidenceReducer::new(
        cardinalities.into_boxed_slice(),
        shape,
        representative,
        false,
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context("cannot construct exact evidence reducer", error)
    })
}

fn require_reducer_universe(
    exact: &ExactEvidenceReducer,
    header: &ExploreRunHeader,
) -> Result<(), ExactStreamCoordinatorError> {
    if exact.universe_case_count() != header.case_universe().case_count() {
        return Err(ExactStreamCoordinatorError::invalid(format!(
            "exact reducer universe {} disagrees with checked stream universe {}",
            exact.universe_case_count(),
            header.case_universe().case_count()
        )));
    }
    Ok(())
}

fn verify_reducer_frontier(
    exact: &ExactEvidenceReducer,
    stream: &ExploreRunStream,
) -> Result<(), ExactStreamCoordinatorError> {
    let universe_case_count = exact.universe_case_count();
    let closed_case_count = exact.closed_case_count();
    let open_case_count = universe_case_count
        .checked_sub(closed_case_count)
        .ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "exact reducer closed support exceeds its universe",
            )
        })?;
    if universe_case_count != stream.header().case_universe().case_count()
        || open_case_count != stream.frontier().open_cases().case_count()
        || closed_case_count
            != stream
                .header()
                .case_universe()
                .case_count()
                .checked_sub(stream.frontier().open_cases().case_count())
                .ok_or_else(|| {
                    ExactStreamCoordinatorError::invalid(
                        "open case frontier exceeds the bound universe",
                    )
                })?
    {
        return Err(ExactStreamCoordinatorError::invalid(
            "exact reducer coverage disagrees with the authenticated case frontier",
        ));
    }
    Ok(())
}

/// Reconstruct the mechanism target frontier from already authenticated case
/// classification. This is derived state, not a second journal fact: confirmed
/// matches authorize early `scope_open` replay, and complete classification
/// deterministically seals the same support as the exact target DAG.
fn synchronize_mechanism_target_knowledge(
    exact: &ExactEvidenceReducer,
    mechanism: Option<&mut MechanismEvidenceReducerV1>,
    materialize_exact_target: bool,
) -> Result<(), ExactStreamCoordinatorError> {
    let Some(mechanism) = mechanism else {
        return Ok(());
    };
    let confirmed = exact.confirmed_admissible_match_support();
    if mechanism.known_target_support() != &confirmed {
        let prepared = mechanism
            .prepare_known_target_support(confirmed.clone())
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot synchronize confirmed matching support for mechanism replay",
                    error,
                )
            })?;
        mechanism.apply_prepared_known_target_support(prepared);
    }

    if materialize_exact_target {
        let Some(authoritative) = exact.authoritative_admissible_match_support() else {
            return Ok(());
        };
        if authoritative.support() != &confirmed {
            return Err(ExactStreamCoordinatorError::invalid(
                "closure-gated matching support disagrees with confirmed matching support",
            ));
        }
        if !mechanism.has_exact_target() {
            let prepared = mechanism
                .prepare_exact_target_from_known_support(&authoritative)
                .map_err(|error| {
                    if error.is_snapshot_capacity() {
                        ExactStreamCoordinatorError::snapshot_capacity(
                            "cannot materialize exact mechanism target from closed case evidence",
                            error,
                        )
                    } else {
                        ExactStreamCoordinatorError::context(
                            "cannot seal exact mechanism target from closed case evidence",
                            error,
                        )
                    }
                })?;
            mechanism.apply_prepared_exact_target(prepared);
        }
    }
    Ok(())
}

fn apply_mechanism_replay(
    store: &ExploreRunStreamStore,
    mechanism: Option<&mut MechanismEvidenceReducerV1>,
    request: Option<&CheckedMechanismObservationRequestV1>,
    header: &ExploreRunHeader,
    payload: &CanonicalRunRecordPayload,
) -> Result<(), ExactStreamCoordinatorError> {
    let CanonicalRunRecordPayload::SemanticObservation {
        producer_kind: ObservationEvidenceKind::MechanismObserved,
        semantic_facts,
        validation_receipt_hash,
        ..
    } = payload
    else {
        return Ok(());
    };
    let request = request.ok_or_else(|| {
        ExactStreamCoordinatorError::invalid(
            "journal contains mechanism evidence but sequence-zero identity defers mechanisms",
        )
    })?;
    let mechanism = mechanism.ok_or_else(|| {
        ExactStreamCoordinatorError::invalid(
            "mechanism-enabled stream has no mechanism evidence reducer",
        )
    })?;
    let bytes = store
        .read_blob(
            MECHANISM_OBSERVATION_BLOB_KIND_V1,
            &validation_receipt_hash.to_lowercase_hex(),
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context("cannot read mechanism observation blob", error)
        })?;
    if content_digest(&bytes) != *validation_receipt_hash {
        return Err(ExactStreamCoordinatorError::invalid(
            "mechanism observation blob bytes disagree with their journal commitment",
        ));
    }
    let proposal = decode_mechanism_observation_batch_v1(request, &bytes).map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot decode canonical mechanism observation blob",
            error,
        )
    })?;
    let validated = restore_committed_mechanism_batch_v1(request, proposal, |validated| {
        let projected = mechanism_evidence_projection(header.case_universe(), request, validated)
            .map_err(|error| error.to_string())?;
        if projected.as_slice() != semantic_facts.as_ref() {
            return Err(
                "mechanism blob disagrees with its normalized semantic evidence facts".to_string(),
            );
        }
        Ok(())
    })
    .map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot restore coordinator-committed mechanism observations",
            error,
        )
    })?;
    let prepared = mechanism
        .prepare_observation_batch(validated)
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "replayed mechanism observations conflict with reducer state",
                error,
            )
        })?;
    mechanism.apply_prepared_observation_batch(prepared);
    Ok(())
}

fn apply_exact_replay(
    store: &ExploreRunStreamStore,
    exact: &mut ExactEvidenceReducer,
    header: &ExploreRunHeader,
    frontier_before: &RequiredFrontier,
    replay_closure: RequiredObligationId,
    payload: &CanonicalRunRecordPayload,
) -> Result<(), ExactStreamCoordinatorError> {
    match payload {
        CanonicalRunRecordPayload::CoveragePlanAccepted { plan, .. }
            if plan.certified_closed().is_empty() =>
        {
            let bytes = store
                .read_blob(
                    SOURCE_CANDIDATE_BLOB_V1,
                    &plan.proof_receipt_hash().to_lowercase_hex(),
                )
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot verify empty-coverage source-proof receipt blob",
                        error,
                    )
                })?;
            decode_source_proof_candidate_ranks_v1(&bytes, header.case_universe().case_count())
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot decode empty-coverage source-proof receipt blob",
                        error,
                    )
                })?;
            Ok(())
        }
        CanonicalRunRecordPayload::CoveragePlanAccepted { plan, .. }
            if !plan.certified_closed().is_empty() =>
        {
            let bytes = store
                .read_blob(
                    EXACT_REGION_BLOB_V1,
                    &plan.proof_receipt_hash().to_lowercase_hex(),
                )
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot read exact coverage region blob",
                        error,
                    )
                })?;
            let proposal = decode_exact_closed_region_batch_v1(&bytes).map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot decode exact coverage region blob",
                    error,
                )
            })?;
            let validated = restore_coordinator_committed_region_batch_v1(proposal, |validated| {
                let projection = region_evidence_projection(header, validated)
                    .map_err(|error| error.to_string())?;
                if &projection.support != plan.certified_closed()
                    || projection.facts.as_slice() != plan.semantic_facts()
                {
                    return Err(
                        "exact coverage region blob disagrees with its committed coverage plan"
                            .to_string(),
                    );
                }
                Ok(())
            })
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot restore coordinator-committed coverage regions",
                    error,
                )
            })?;
            let prepared = exact
                .prepare_closed_region_batch(validated)
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "replayed exact coverage conflicts with reducer state",
                        error,
                    )
                })?;
            exact.apply_prepared_closed_region_batch(prepared);
            Ok(())
        }
        CanonicalRunRecordPayload::FrontierTransition {
            producer_kind,
            newly_closed,
            semantic_facts,
            validation_receipt_hash,
            ..
        } => match producer_kind {
            FrontierEvidenceKind::SingletonClassification
            | FrontierEvidenceKind::BoundedExactBatchClassification
            | FrontierEvidenceKind::ProbeCandidateBatchClassification
            | FrontierEvidenceKind::ExactExhaustion => {
                let bytes = store
                    .read_blob(
                        EXACT_OBSERVATION_BLOB_V1,
                        &validation_receipt_hash.to_lowercase_hex(),
                    )
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot read exact observation blob",
                            error,
                        )
                    })?;
                let proposal = decode_exact_case_observation_batch_v1(&bytes).map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot decode exact observation blob",
                        error,
                    )
                })?;
                let canonical_blob_bytes = bytes.len();
                let validated = restore_coordinator_committed_observation_batch_v1(
                    proposal,
                    |validated| {
                        let projection = observation_evidence_projection(header, validated)
                            .map_err(|error| error.to_string())?;
                        require_exact_frontier_projection(
                            newly_closed,
                            semantic_facts,
                            &projection,
                        )
                        .map_err(|error| error.to_string())?;
                        if *producer_kind
                            == FrontierEvidenceKind::SingletonClassification
                            && projection.support.case_count() != 1
                        {
                            return Err(
                                "replayed singleton classification closes more than one CaseId"
                                .to_string(),
                            );
                        }
                        if *producer_kind
                            == FrontierEvidenceKind::BoundedExactBatchClassification
                            && projection.support.case_count() < 2
                        {
                            return Err(
                                "replayed bounded exact batch closes fewer than two CaseIds"
                                    .to_string(),
                            );
                        }
                        if *producer_kind
                            == FrontierEvidenceKind::ProbeCandidateBatchClassification
                            && projection.support.case_count() == 0
                        {
                            return Err(
                                "replayed source-probe candidate batch closes no CaseId"
                                    .to_string(),
                            );
                        }
                        if *producer_kind
                            == FrontierEvidenceKind::ProbeCandidateBatchClassification
                            && projection.support.case_count()
                                > u128::from(EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP)
                        {
                            return Err(
                                "replayed source-probe candidate batch exceeds the first-generation CaseId cap"
                                    .to_string(),
                            );
                        }
                        if *producer_kind
                            == FrontierEvidenceKind::ProbeCandidateBatchClassification
                            && projection.support.case_count() > 1
                            && canonical_blob_bytes
                                > EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1
                        {
                            return Err(
                                "replayed source-probe candidate batch exceeds its canonical byte target"
                                    .to_string(),
                            );
                        }
                        if *producer_kind
                            == FrontierEvidenceKind::BoundedExactBatchClassification
                            && projection.support.case_count()
                                > u128::from(EXACT_STREAM_FIRST_GENERATION_BATCH_CASE_CAP)
                        {
                            return Err(
                                "replayed bounded exact batch exceeds the first-generation CaseId cap"
                                    .to_string(),
                            );
                        }
                        if *producer_kind
                            == FrontierEvidenceKind::BoundedExactBatchClassification
                            && canonical_blob_bytes
                                > EXACT_STREAM_OBSERVATION_BATCH_TARGET_BYTES_V1
                        {
                            return Err(
                                "replayed bounded exact batch exceeds its canonical byte target"
                                    .to_string(),
                            );
                        }
                        if *producer_kind == FrontierEvidenceKind::ExactExhaustion {
                            let residual = frontier_before
                                .open_cases()
                                .subtract_exact(newly_closed.open_cases())
                                .map_err(|error| error.to_string())?;
                            if !residual.is_empty() {
                                return Err(
                                    "replayed exact-exhaustion event leaves CaseIds open"
                                        .to_string(),
                                );
                            }
                        }
                        Ok(())
                    },
                )
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot restore coordinator-committed observations",
                        error,
                    )
                })?;
                let prepared = exact
                    .prepare_observation_batch(validated)
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "replayed observations conflict with exact reducer state",
                            error,
                        )
                    })?;
                exact.apply_prepared_observation_batch(prepared);
                Ok(())
            }
            FrontierEvidenceKind::CertifiedRegionClassification => {
                let bytes = store
                    .read_blob(
                        EXACT_REGION_BLOB_V1,
                        &validation_receipt_hash.to_lowercase_hex(),
                    )
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot read exact certified-region blob",
                            error,
                        )
                    })?;
                let proposal = decode_exact_closed_region_batch_v1(&bytes).map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot decode exact certified-region blob",
                        error,
                    )
                })?;
                let validated =
                    restore_coordinator_committed_region_batch_v1(proposal, |validated| {
                        let projection = region_evidence_projection(header, validated)
                            .map_err(|error| error.to_string())?;
                        require_exact_frontier_projection(newly_closed, semantic_facts, &projection)
                            .map_err(|error| error.to_string())
                    })
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot restore coordinator-committed certified regions",
                            error,
                        )
                    })?;
                let prepared = exact
                    .prepare_closed_region_batch(validated)
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "replayed certified regions conflict with exact reducer state",
                            error,
                        )
                    })?;
                exact.apply_prepared_closed_region_batch(prepared);
                Ok(())
            }
            FrontierEvidenceKind::RepresentativeSelectionClosed => {
                if !newly_closed.open_cases().is_empty() {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "non-classification frontier event unexpectedly closes CaseIds",
                    ));
                }
                let bytes = store
                    .read_blob(
                        EXACT_REPLAY_CLOSURE_BLOB_KIND_V1,
                        &validation_receipt_hash.to_lowercase_hex(),
                    )
                    .map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot read exact replay-closure manifest",
                            error,
                        )
                    })?;
                let manifest =
                    decode_exact_replay_closure_manifest_v1(&bytes).map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot decode exact replay-closure manifest",
                            error,
                        )
                    })?;
                let validation =
                    validate_exact_replay_closure_v1(exact, &manifest).map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "durable exact replay-closure manifest is invalid",
                            error,
                        )
                    })?;
                let expected_obligations = BTreeSet::from([replay_closure]);
                let expected_fact = SemanticEvidenceFact::new(
                    SemanticEvidenceLayer::RepresentativeSelection,
                    validation.normalized_witness_digest(),
                    SemanticEvidenceSubject::obligations([replay_closure]).map_err(|error| {
                        ExactStreamCoordinatorError::context(
                            "cannot reconstruct replay-obligation subject",
                            error,
                        )
                    })?,
                )
                .map_err(|error| {
                    ExactStreamCoordinatorError::context(
                        "cannot reconstruct replay-obligation fact",
                        error,
                    )
                })?;
                if newly_closed.open_obligations() != &expected_obligations
                    || semantic_facts.len() != 1
                    || semantic_facts[0] != expected_fact
                {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "exact replay-closure manifest disagrees with its committed obligation transition",
                    ));
                }
                Ok(())
            }
            FrontierEvidenceKind::MechanismTargetClosed => {
                if !newly_closed.open_cases().is_empty() {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "non-classification frontier event unexpectedly closes CaseIds",
                    ));
                }
                if newly_closed.open_obligations().contains(&replay_closure) {
                    return Err(ExactStreamCoordinatorError::invalid(
                        "mechanism evidence cannot close the exact representative replay obligation",
                    ));
                }
                Ok(())
            }
        },
        _ => Ok(()),
    }
}

fn proposal_from_validated_regions(
    validated: &ValidatedExactClosedRegionBatchV1,
) -> Result<ExactClosedRegionBatchProposalV1, ExactStreamCoordinatorError> {
    ExactClosedRegionBatchProposalV1::new(
        validated
            .regions()
            .iter()
            .map(|region| region.proposal().clone())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context(
            "cannot reconstruct canonical exact region proposal",
            error,
        )
    })
}

struct ExactEvidenceProjection {
    support: ExactCaseSupport,
    facts: Vec<SemanticEvidenceFact>,
}

/// Semantic identity deliberately excludes CaseId, batch boundaries and
/// validator receipts. Those remain ordered-journal provenance; equal facts
/// from disjoint batches therefore merge into one authenticated support fiber.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MechanismSemanticFactKindV1 {
    Signature([u8; 32]),
    PermanentlyUntraced(MechanismPermanentUntracedReasonV1),
    BinAssignment {
        signature: [u8; 32],
        field_name: Box<str>,
        outcome: MechanismBinAssignmentOutcomeV1,
    },
}

fn mechanism_evidence_projection(
    universe: &super::run_stream::ExploreCaseUniverse,
    request: &CheckedMechanismObservationRequestV1,
    batch: &ValidatedMechanismObservationBatchV1,
) -> Result<Vec<SemanticEvidenceFact>, ExactStreamCoordinatorError> {
    if request.observation.axis_cardinalities.as_ref() != universe.axis_cardinalities() {
        return Err(ExactStreamCoordinatorError::invalid(
            "mechanism semantic facts belong to another case universe",
        ));
    }
    let semantic_entries = batch
        .proposal()
        .observations()
        .iter()
        .try_fold(
            batch.proposal().observations().len(),
            |total, observation| {
                let assignments = match &observation.outcome {
                    MechanismCaseObservationOutcomeProposalV1::Observed {
                        bin_assignments, ..
                    } => bin_assignments.len(),
                    MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(_) => 0,
                };
                total.checked_add(assignments)
            },
        )
        .ok_or_else(|| {
            ExactStreamCoordinatorError::invalid(
                "mechanism semantic-fact entry count exceeds usize::MAX",
            )
        })?;
    if semantic_entries > MAX_NORMALIZED_SEMANTIC_FACTS_PER_BATCH {
        return Err(ExactStreamCoordinatorError::mechanism_capacity(
            "cannot normalize mechanism batch",
            format!(
                "requires {semantic_entries} semantic-fact entries; fixed limit is {MAX_NORMALIZED_SEMANTIC_FACTS_PER_BATCH}"
            ),
        ));
    }
    let mut grouped = BTreeMap::<MechanismSemanticFactKindV1, Vec<(u128, u128)>>::new();
    for observation in batch.proposal().observations() {
        let end = observation.case_id.rank.checked_add(1).ok_or_else(|| {
            ExactStreamCoordinatorError::invalid("mechanism CaseId rank overflows u128")
        })?;
        match &observation.outcome {
            MechanismCaseObservationOutcomeProposalV1::Observed {
                signature,
                bin_assignments,
            } => {
                let signature = signature.digest_bytes();
                grouped
                    .entry(MechanismSemanticFactKindV1::Signature(signature))
                    .or_default()
                    .push((observation.case_id.rank, end));
                for assignment in bin_assignments.iter() {
                    grouped
                        .entry(MechanismSemanticFactKindV1::BinAssignment {
                            signature,
                            field_name: assignment.field_name.clone(),
                            outcome: assignment.outcome,
                        })
                        .or_default()
                        .push((observation.case_id.rank, end));
                }
            }
            MechanismCaseObservationOutcomeProposalV1::PermanentlyUntraced(reason) => {
                grouped
                    .entry(MechanismSemanticFactKindV1::PermanentlyUntraced(*reason))
                    .or_default()
                    .push((observation.case_id.rank, end));
            }
        }
    }

    let mut facts = Vec::with_capacity(grouped.len());
    for (kind, intervals) in grouped {
        let support = ExactCaseSupport::new(universe, intervals).map_err(|error| {
            ExactStreamCoordinatorError::context(
                "cannot construct mechanism semantic support",
                error,
            )
        })?;
        facts.push(
            SemanticEvidenceFact::new(
                SemanticEvidenceLayer::MechanismObservation,
                mechanism_semantic_fact_digest(request, &kind),
                SemanticEvidenceSubject::cases(support),
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot construct normalized mechanism semantic fact",
                    error,
                )
            })?,
        );
    }
    facts.sort_by_key(SemanticEvidenceFact::normalized_content_hash);
    Ok(facts)
}

fn mechanism_semantic_fact_digest(
    request: &CheckedMechanismObservationRequestV1,
    kind: &MechanismSemanticFactKindV1,
) -> CanonicalDigest {
    let mut hasher = Sha256::new();
    hash_mechanism_fact_segment(
        &mut hasher,
        b"futuruna.explore.normalized-mechanism-fact.v1",
    );
    hash_mechanism_fact_segment(&mut hasher, &request.id.digest_bytes());
    match kind {
        MechanismSemanticFactKindV1::Signature(signature) => {
            hash_mechanism_fact_segment(&mut hasher, b"signature");
            hash_mechanism_fact_segment(&mut hasher, signature);
        }
        MechanismSemanticFactKindV1::PermanentlyUntraced(reason) => {
            hash_mechanism_fact_segment(&mut hasher, b"permanently-untraced");
            hash_mechanism_fact_segment(
                &mut hasher,
                match reason {
                    MechanismPermanentUntracedReasonV1::ReplayUnavailable => b"replay-unavailable",
                    MechanismPermanentUntracedReasonV1::ObservationUnsupported => {
                        b"observation-unsupported"
                    }
                },
            );
        }
        MechanismSemanticFactKindV1::BinAssignment {
            signature,
            field_name,
            outcome,
        } => {
            hash_mechanism_fact_segment(&mut hasher, b"bin-assignment");
            hash_mechanism_fact_segment(&mut hasher, signature);
            hash_mechanism_fact_segment(&mut hasher, field_name.as_bytes());
            match outcome {
                MechanismBinAssignmentOutcomeV1::Binned(bin) => {
                    hash_mechanism_fact_segment(&mut hasher, b"binned");
                    hash_mechanism_fact_segment(&mut hasher, &bin.lower_inclusive.to_le_bytes());
                    hash_mechanism_fact_segment(&mut hasher, &bin.upper_exclusive.to_le_bytes());
                }
                MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins => {
                    hash_mechanism_fact_segment(&mut hasher, b"outside-declared-bins");
                }
                MechanismBinAssignmentOutcomeV1::ReplayUnavailable => {
                    hash_mechanism_fact_segment(&mut hasher, b"value-replay-unavailable");
                }
                MechanismBinAssignmentOutcomeV1::ObservationUnsupported => {
                    hash_mechanism_fact_segment(&mut hasher, b"value-observation-unsupported");
                }
            }
        }
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    CanonicalDigest::from_sha256_bytes(bytes)
}

fn hash_mechanism_fact_segment(hasher: &mut Sha256, segment: &[u8]) {
    hasher.update((segment.len() as u64).to_le_bytes());
    hasher.update(segment);
}

fn observation_evidence_projection(
    header: &ExploreRunHeader,
    batch: &ValidatedExactCaseObservationBatchV1,
) -> Result<ExactEvidenceProjection, ExactStreamCoordinatorError> {
    exact_evidence_projection(
        header.case_universe(),
        batch.observations().iter().map(|observation| {
            (
                observation.proposal().case_id.rank,
                observation.proposal().case_id.rank + 1,
                observation.semantic_fact_digest().bytes(),
            )
        }),
    )
}

fn region_evidence_projection(
    header: &ExploreRunHeader,
    batch: &ValidatedExactClosedRegionBatchV1,
) -> Result<ExactEvidenceProjection, ExactStreamCoordinatorError> {
    exact_evidence_projection(
        header.case_universe(),
        batch.regions().iter().map(|region| {
            (
                region.proposal().start_rank,
                region.proposal().end_rank_exclusive,
                region.semantic_fact_digest().bytes(),
            )
        }),
    )
}

fn exact_evidence_projection(
    universe: &super::run_stream::ExploreCaseUniverse,
    entries: impl IntoIterator<Item = (u128, u128, [u8; 32])>,
) -> Result<ExactEvidenceProjection, ExactStreamCoordinatorError> {
    let entries = entries.into_iter().collect::<Vec<_>>();
    let support = ExactCaseSupport::new(
        universe,
        entries.iter().map(|(start, end, _)| (*start, *end)),
    )
    .map_err(|error| {
        ExactStreamCoordinatorError::context("cannot construct exact semantic support", error)
    })?;
    let mut grouped = BTreeMap::<[u8; 32], Vec<(u128, u128)>>::new();
    for (start, end, digest) in entries {
        grouped.entry(digest).or_default().push((start, end));
    }
    let mut facts = Vec::with_capacity(grouped.len());
    for (digest, intervals) in grouped {
        let support = ExactCaseSupport::new(universe, intervals).map_err(|error| {
            ExactStreamCoordinatorError::context("cannot construct grouped semantic support", error)
        })?;
        facts.push(
            SemanticEvidenceFact::new(
                SemanticEvidenceLayer::CaseClassification,
                CanonicalDigest::from_sha256_bytes(digest),
                SemanticEvidenceSubject::cases(support),
            )
            .map_err(|error| {
                ExactStreamCoordinatorError::context(
                    "cannot construct normalized case-classification fact",
                    error,
                )
            })?,
        );
    }
    Ok(ExactEvidenceProjection { support, facts })
}

fn require_exact_frontier_projection(
    newly_closed: &RequiredFrontier,
    semantic_facts: &[SemanticEvidenceFact],
    projection: &ExactEvidenceProjection,
) -> Result<(), ExactStreamCoordinatorError> {
    if !newly_closed.open_obligations().is_empty()
        || newly_closed.open_cases() != &projection.support
        || semantic_facts != projection.facts.as_slice()
    {
        return Err(ExactStreamCoordinatorError::invalid(
            "exact evidence blob disagrees with its committed frontier transition",
        ));
    }
    Ok(())
}

fn require_source_proof_identity(
    header: &ExploreRunHeader,
    producer: Option<super::stream_proof::SourceProofProducerIdentityV1>,
) -> Result<(), ExactStreamCoordinatorError> {
    let Some(producer) = producer else {
        return Ok(());
    };
    if producer.analysis_program_digest() != header.identity().analysis_program_hash().bytes()
        || producer.query_digest() != header.identity().query_hash().bytes()
    {
        return Err(ExactStreamCoordinatorError::invalid(
            "source-proof producer identity does not match the checked stream header",
        ));
    }
    Ok(())
}

fn verify_historical_lease(
    store: &ExploreRunStreamStore,
    lease: FencedWriterLease,
) -> Result<(), ExactStreamCoordinatorError> {
    let identity =
        canonical_writer_fence_identity(lease.run_id(), lease.generation(), lease.writer_id());
    store
        .verify_historical_writer_fence(
            lease.generation(),
            &lease.fence_receipt_hash().to_lowercase_hex(),
            &identity,
        )
        .map_err(|error| {
            ExactStreamCoordinatorError::context(
                "historical record has no matching durable writer fence",
                error,
            )
        })
}

fn require_receipt_matches_lease(
    receipt: &ExploreWriterFenceReceipt,
    lease: FencedWriterLease,
    expected_identity: &[u8],
) -> Result<(), ExactStreamCoordinatorError> {
    if receipt.generation() != lease.generation()
        || receipt.receipt_hash() != lease.fence_receipt_hash().to_lowercase_hex()
        || receipt.writer_lease_identity() != expected_identity
    {
        return Err(ExactStreamCoordinatorError::invalid(
            "durable writer-fence receipt disagrees with the minted lease",
        ));
    }
    Ok(())
}

fn canonical_writer_fence_identity(
    run_id: super::run_stream::ExploreRunId,
    generation: NonZeroU64,
    writer_id: ExploreWriterId,
) -> Vec<u8> {
    let run_id = run_id.to_lowercase_hex();
    let mut bytes = Vec::with_capacity(WRITER_FENCE_IDENTITY_V1.len() + run_id.len() + 8 + 32);
    bytes.extend_from_slice(WRITER_FENCE_IDENTITY_V1);
    bytes.extend_from_slice(run_id.as_bytes());
    bytes.extend_from_slice(&generation.get().to_le_bytes());
    bytes.extend_from_slice(&writer_id.identity().bytes());
    bytes
}

fn content_digest(bytes: &[u8]) -> CanonicalDigest {
    CanonicalDigest::from_sha256_bytes(Sha256::digest(bytes).into())
}

fn canonical_probe_fallback_proof_set_id(
    header: &ExploreRunHeader,
    candidate_blob: CanonicalDigest,
) -> CanonicalDigest {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_PROBE_FALLBACK_PROOF_SET_V1);
    hasher.update(header.commitment_hash().bytes());
    hasher.update(candidate_blob.bytes());
    CanonicalDigest::from_sha256_bytes(hasher.finalize().into())
}

#[cfg(unix)]
fn os_random_nonzero_digest(subject: &str) -> Result<CanonicalDigest, ExactStreamCoordinatorError> {
    let mut source = File::open("/dev/urandom").map_err(|error| {
        ExactStreamCoordinatorError::context(
            &format!("cannot open operating-system entropy for {subject}"),
            error,
        )
    })?;
    loop {
        let mut bytes = [0_u8; 32];
        source.read_exact(&mut bytes).map_err(|error| {
            ExactStreamCoordinatorError::context(
                &format!("cannot read operating-system entropy for {subject}"),
                error,
            )
        })?;
        if bytes != [0; 32] {
            return Ok(CanonicalDigest::from_sha256_bytes(bytes));
        }
    }
}

#[cfg(not(unix))]
fn os_random_nonzero_digest(subject: &str) -> Result<CanonicalDigest, ExactStreamCoordinatorError> {
    Err(ExactStreamCoordinatorError::invalid(format!(
        "secure operating-system entropy for {subject} is not implemented on this platform"
    )))
}
