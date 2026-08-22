//! Strict canonical binary codec for durable Explore run records.
//!
//! A record is self-delimiting and contains both the committed envelope and
//! the complete typed reducer payload. All integers are fixed-width little
//! endian, all hashes are raw 32-byte SHA-256 values, and all collections carry
//! a `u32` element count. The top-level layout is:
//!
//! ```text
//! magic[16] | domain[32] | version:u16 | flags:u16 |
//! run_id[32] | sequence:u64 | previous_head[32] | journal_head[32] |
//! evidence_root[32] | event_kind[3] | payload_hash[32] |
//! lease_generation:u64 | lease_id[32] | payload_bytes:u32 | payload[..]
//! ```
//!
//! Payload tag `0` is `RunOpened`, `1` discovery, `2` semantic observation,
//! `3` accepted coverage plan, `4` frontier transition, `5` pause, `6` resume,
//! `7` recovery, and `8` terminal seal. Nested values are encoded in their
//! constructor argument order. A support is `count:u32`, repeated
//! `(start:u128, end_exclusive:u128)`, derived `case_count:u128`, and its
//! identity hash. A frontier is a support, sorted obligation hashes, and its
//! identity hash. A semantic fact is `layer:u8`, content hash, `subject:u8`,
//! then support/obligations/no bytes for cases/obligations/global. A frontier
//! transition carries the previous commitment, the complete bounded closure
//! delta, and the derived next commitment; it never repeats either accumulated
//! frontier body. A lease is
//! run ID, generation, writer ID, fence receipt, and derived lease ID.
//!
//! Bounds are record-wide, not merely per nested collection: 64 MiB encoded
//! bytes, 4,096 axes, 1,000,000 support intervals, 262,144 obligation entries,
//! and 262,144 semantic facts. Counts and minimum encoded item sizes are
//! checked before collection allocation.
//!
//! Decoding reconstructs semantic values only through the validating
//! constructors in `run_stream`, checks every redundantly stored identity, and
//! finally requires exact byte equality with a fresh canonical encoding.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;

use super::run_stream::{
    CanonicalDigest, CanonicalRunRecordPayload, CommittedRunEvent, ControlEventKind, CoveragePlan,
    DiscoveryEventKind, EvidenceEventKind, EvidenceRoot, ExactCaseSupport, ExploreCaseUniverse,
    ExploreRunHeader, ExploreRunId, ExploreRunIdentity, ExploreRunNonce, ExploreRunSchemas,
    ExploreRunStreamError, ExploreWriterId, FencedWriterLease, FrontierEvidenceKind, JournalHead,
    ObservationEvidenceKind, PauseReason, RequiredFrontier, RequiredObligationId, RunEventKind,
    SemanticEvidenceFact, SemanticEvidenceLayer, SemanticEvidenceSubject, TerminalMethodHash,
    TerminalPayloadHash, TerminalSealKind,
};

const RECORD_MAGIC: &[u8; 16] = b"FTRNEXPLOREREC3!";
const RECORD_DOMAIN: &[u8; 32] = b"futuruna.explore.record.codec.v3";

pub(crate) const RUN_STREAM_RECORD_CODEC_VERSION: u16 = 3;
pub(crate) const RUN_STREAM_RECORD_MAX_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const RUN_STREAM_RECORD_MAX_AXES: usize = 4_096;
pub(crate) const RUN_STREAM_RECORD_MAX_INTERVALS: usize = 1_000_000;
pub(crate) const RUN_STREAM_RECORD_MAX_OBLIGATIONS: usize = 262_144;
pub(crate) const RUN_STREAM_RECORD_MAX_SEMANTIC_FACTS: usize = 262_144;

const RECORD_FLAGS: u16 = 0;

/// One decoded canonical record ready for the pure replay reducer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedRunRecord {
    event: CommittedRunEvent,
    payload: CanonicalRunRecordPayload,
}

impl DecodedRunRecord {
    pub(crate) fn event(&self) -> &CommittedRunEvent {
        &self.event
    }

    pub(crate) fn payload(&self) -> &CanonicalRunRecordPayload {
        &self.payload
    }

    pub(crate) fn into_parts(self) -> (CommittedRunEvent, CanonicalRunRecordPayload) {
        (self.event, self.payload)
    }
}

/// Encode one already-prepared or replay-validated committed record.
pub(crate) fn encode_record(
    event: &CommittedRunEvent,
    payload: &CanonicalRunRecordPayload,
) -> Result<Vec<u8>, RunStreamCodecError> {
    validate_envelope(event, payload)?;

    let mut encoder = Encoder::new();
    encoder.bytes(RECORD_MAGIC)?;
    encoder.bytes(RECORD_DOMAIN)?;
    encoder.u16(RUN_STREAM_RECORD_CODEC_VERSION)?;
    encoder.u16(RECORD_FLAGS)?;
    encoder.run_id("envelope.run_id", event.run_id())?;
    encoder.u64(event.sequence())?;
    encoder.journal_head(
        "envelope.previous_journal_head",
        event.previous_journal_head(),
    )?;
    encoder.journal_head("envelope.journal_head", event.journal_head())?;
    encoder.evidence_root("envelope.evidence_root", event.evidence_root())?;
    encode_event_kind(&mut encoder, event.kind())?;
    encoder.digest(
        "envelope.canonical_payload_hash",
        event.canonical_payload_hash(),
    )?;
    encoder.u64(event.lease_generation().get())?;
    encoder.digest("envelope.lease_id", event.lease_id_hash())?;

    let payload_length_offset = encoder.len();
    encoder.u32(0)?;
    let payload_offset = encoder.len();
    encode_payload(&mut encoder, payload)?;
    let payload_length =
        encoder
            .len()
            .checked_sub(payload_offset)
            .ok_or(RunStreamCodecError::LengthOverflow {
                field: "payload_bytes",
            })?;
    let payload_length =
        u32::try_from(payload_length).map_err(|_| RunStreamCodecError::LengthOverflow {
            field: "payload_bytes",
        })?;
    encoder.patch_u32(payload_length_offset, payload_length)?;
    Ok(encoder.finish())
}

/// Decode the unique sequence-zero `RunOpened` record.
pub(crate) fn decode_genesis_record(bytes: &[u8]) -> Result<DecodedRunRecord, RunStreamCodecError> {
    decode_record(bytes, DecodeContext::Genesis)
}

/// Decode a non-genesis record against its already validated immutable header.
pub(crate) fn decode_later_record(
    bytes: &[u8],
    header: &ExploreRunHeader,
) -> Result<DecodedRunRecord, RunStreamCodecError> {
    decode_record(bytes, DecodeContext::Later(header))
}

#[derive(Clone, Copy)]
enum DecodeContext<'a> {
    Genesis,
    Later(&'a ExploreRunHeader),
}

fn decode_record(
    bytes: &[u8],
    context: DecodeContext<'_>,
) -> Result<DecodedRunRecord, RunStreamCodecError> {
    if bytes.len() > RUN_STREAM_RECORD_MAX_BYTES {
        return Err(RunStreamCodecError::RecordTooLarge {
            actual: bytes.len(),
            maximum: RUN_STREAM_RECORD_MAX_BYTES,
        });
    }

    let mut decoder = Decoder::new(bytes);
    let magic = decoder.array::<16>()?;
    if &magic != RECORD_MAGIC {
        return Err(RunStreamCodecError::InvalidMagic);
    }
    let domain = decoder.array::<32>()?;
    if &domain != RECORD_DOMAIN {
        return Err(RunStreamCodecError::InvalidDomain);
    }
    let version = decoder.u16()?;
    if version != RUN_STREAM_RECORD_CODEC_VERSION {
        return Err(RunStreamCodecError::UnsupportedVersion { version });
    }
    let flags = decoder.u16()?;
    if flags != RECORD_FLAGS {
        return Err(RunStreamCodecError::UnsupportedFlags { flags });
    }

    let run_id = decoder.run_id()?;
    let sequence = decoder.u64()?;
    match context {
        DecodeContext::Genesis if sequence != 0 => {
            return Err(RunStreamCodecError::SequenceForWrongDecodeApi {
                api: "decode_genesis_record",
                sequence,
            });
        }
        DecodeContext::Later(header) => {
            if sequence == 0 {
                return Err(RunStreamCodecError::SequenceForWrongDecodeApi {
                    api: "decode_later_record",
                    sequence,
                });
            }
            if run_id != header.run_id() {
                return Err(RunStreamCodecError::RunIdMismatch {
                    field: "envelope.run_id",
                });
            }
        }
        DecodeContext::Genesis => {}
    }
    let previous_journal_head = decoder.journal_head()?;
    let journal_head = decoder.journal_head()?;
    let evidence_root = decoder.evidence_root()?;
    let kind = decode_event_kind(&mut decoder)?;
    let canonical_payload_hash = decoder.digest("envelope.canonical_payload_hash")?;
    let lease_generation = nonzero_u64("envelope.lease_generation", decoder.u64()?)?;
    let lease_id_hash = decoder.digest("envelope.lease_id")?;
    let declared_payload_length = decoder.u32()?;
    let remaining = decoder.remaining();
    if declared_payload_length as usize != remaining {
        return Err(RunStreamCodecError::PayloadLengthMismatch {
            declared: declared_payload_length,
            remaining,
        });
    }

    let payload = decode_payload(&mut decoder, context)?;
    if decoder.remaining() != 0 {
        return Err(RunStreamCodecError::TrailingBytes {
            field: "payload",
            remaining: decoder.remaining(),
        });
    }

    match (&context, &payload) {
        (DecodeContext::Genesis, CanonicalRunRecordPayload::RunOpened { header, lease }) => {
            if header.run_id() != run_id {
                return Err(RunStreamCodecError::RunIdMismatch {
                    field: "payload.header.run_id",
                });
            }
            if lease.run_id() != header.run_id() {
                return Err(RunStreamCodecError::RunIdMismatch {
                    field: "payload.lease.run_id",
                });
            }
            if lease.generation().get() != 1 {
                return Err(RunStreamCodecError::InvalidGenesisLeaseGeneration {
                    actual: lease.generation().get(),
                });
            }
        }
        (DecodeContext::Genesis, _) => {
            return Err(RunStreamCodecError::PayloadForWrongDecodeApi {
                api: "decode_genesis_record",
            });
        }
        (DecodeContext::Later(_), CanonicalRunRecordPayload::RunOpened { .. }) => {
            return Err(RunStreamCodecError::PayloadForWrongDecodeApi {
                api: "decode_later_record",
            });
        }
        (DecodeContext::Later(_), _) => {}
    }

    let event = CommittedRunEvent::from_decoded_envelope(
        run_id,
        sequence,
        previous_journal_head,
        journal_head,
        evidence_root,
        kind,
        canonical_payload_hash,
        lease_generation,
        lease_id_hash,
        &payload,
    )?;
    let decoded = DecodedRunRecord { event, payload };
    let canonical = encode_record(decoded.event(), decoded.payload())?;
    if canonical.as_slice() != bytes {
        return Err(RunStreamCodecError::NonCanonicalEncoding);
    }
    Ok(decoded)
}

fn validate_envelope(
    event: &CommittedRunEvent,
    payload: &CanonicalRunRecordPayload,
) -> Result<(), RunStreamCodecError> {
    let reconstructed = CommittedRunEvent::from_decoded_envelope(
        event.run_id(),
        event.sequence(),
        event.previous_journal_head(),
        event.journal_head(),
        event.evidence_root(),
        event.kind(),
        event.canonical_payload_hash(),
        event.lease_generation(),
        event.lease_id_hash(),
        payload,
    )?;
    if reconstructed != *event {
        return Err(RunStreamCodecError::EnvelopeMismatch);
    }
    match payload {
        CanonicalRunRecordPayload::RunOpened { header, lease } => {
            if header.run_id() != event.run_id() || lease.run_id() != header.run_id() {
                return Err(RunStreamCodecError::RunIdMismatch { field: "RunOpened" });
            }
            if lease.generation().get() != 1 {
                return Err(RunStreamCodecError::InvalidGenesisLeaseGeneration {
                    actual: lease.generation().get(),
                });
            }
        }
        CanonicalRunRecordPayload::CoveragePlanAccepted { plan, .. } => {
            if plan.run_id() != event.run_id() {
                return Err(RunStreamCodecError::RunIdMismatch {
                    field: "CoveragePlanAccepted.plan",
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn encode_payload(
    encoder: &mut Encoder,
    payload: &CanonicalRunRecordPayload,
) -> Result<(), RunStreamCodecError> {
    match payload {
        CanonicalRunRecordPayload::RunOpened { header, lease } => {
            encoder.u8(0)?;
            encode_header(encoder, header)?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::Discovery {
            kind,
            canonical_discovery_hash,
            lease,
        } => {
            encoder.u8(1)?;
            encoder.u8(discovery_tag(*kind))?;
            encoder.digest(
                "discovery.canonical_discovery_hash",
                *canonical_discovery_hash,
            )?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::SemanticObservation {
            producer_kind,
            semantic_facts,
            validation_receipt_hash,
            lease,
        } => {
            encoder.u8(2)?;
            encoder.u8(observation_tag(*producer_kind))?;
            encode_semantic_facts(encoder, semantic_facts)?;
            encoder.digest(
                "semantic_observation.validation_receipt_hash",
                *validation_receipt_hash,
            )?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::CoveragePlanAccepted { plan, lease } => {
            encoder.u8(3)?;
            encode_coverage_plan(encoder, plan)?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::FrontierTransition {
            producer_kind,
            previous_frontier_commitment,
            newly_closed,
            next_frontier_commitment,
            semantic_facts,
            validation_receipt_hash,
            lease,
        } => {
            encoder.u8(4)?;
            encoder.u8(frontier_tag(*producer_kind))?;
            encoder.digest(
                "frontier_transition.previous_frontier_commitment",
                *previous_frontier_commitment,
            )?;
            encode_frontier(encoder, newly_closed)?;
            encoder.digest(
                "frontier_transition.next_frontier_commitment",
                *next_frontier_commitment,
            )?;
            encode_semantic_facts(encoder, semantic_facts)?;
            encoder.digest(
                "frontier_transition.validation_receipt_hash",
                *validation_receipt_hash,
            )?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::Paused {
            reason,
            previous_journal_head,
            evidence_root,
            lease,
        } => {
            encoder.u8(5)?;
            encoder.u8(pause_tag(*reason))?;
            encoder.journal_head("paused.previous_journal_head", *previous_journal_head)?;
            encoder.evidence_root("paused.evidence_root", *evidence_root)?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::Resumed {
            previous_journal_head,
            evidence_root,
            lease,
        } => {
            encoder.u8(6)?;
            encoder.journal_head("resumed.previous_journal_head", *previous_journal_head)?;
            encoder.evidence_root("resumed.evidence_root", *evidence_root)?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::Recovered {
            previous_journal_head,
            evidence_root,
            lease,
        } => {
            encoder.u8(7)?;
            encoder.journal_head("recovered.previous_journal_head", *previous_journal_head)?;
            encoder.evidence_root("recovered.evidence_root", *evidence_root)?;
            encode_lease(encoder, *lease)?;
        }
        CanonicalRunRecordPayload::TerminalSeal {
            kind,
            journal_head_before_seal,
            evidence_root,
            terminal_payload_hash,
            method_hash,
            lease,
        } => {
            encoder.u8(8)?;
            encoder.u8(terminal_tag(*kind))?;
            encoder.journal_head(
                "terminal_seal.journal_head_before_seal",
                *journal_head_before_seal,
            )?;
            encoder.evidence_root("terminal_seal.evidence_root", *evidence_root)?;
            encoder.terminal_payload_hash(*terminal_payload_hash)?;
            encoder.terminal_method_hash(*method_hash)?;
            encode_lease(encoder, *lease)?;
        }
    }
    Ok(())
}

fn decode_payload(
    decoder: &mut Decoder<'_>,
    context: DecodeContext<'_>,
) -> Result<CanonicalRunRecordPayload, RunStreamCodecError> {
    let tag = decoder.u8()?;
    match tag {
        0 => {
            if !matches!(context, DecodeContext::Genesis) {
                return Err(RunStreamCodecError::PayloadForWrongDecodeApi {
                    api: "decode_later_record",
                });
            }
            let header = decode_header(decoder)?;
            let lease = decode_lease(decoder)?;
            Ok(CanonicalRunRecordPayload::RunOpened { header, lease })
        }
        1 => Ok(CanonicalRunRecordPayload::Discovery {
            kind: decode_discovery(decoder.u8()?)?,
            canonical_discovery_hash: decoder.digest("discovery.canonical_discovery_hash")?,
            lease: decode_lease(decoder)?,
        }),
        2 => {
            let DecodeContext::Later(header) = context else {
                return Err(RunStreamCodecError::PayloadForWrongDecodeApi {
                    api: "decode_genesis_record",
                });
            };
            Ok(CanonicalRunRecordPayload::SemanticObservation {
                producer_kind: decode_observation(decoder.u8()?)?,
                semantic_facts: decode_semantic_facts_with_universe(
                    decoder,
                    header.case_universe(),
                )?,
                validation_receipt_hash: decoder
                    .digest("semantic_observation.validation_receipt_hash")?,
                lease: decode_lease(decoder)?,
            })
        }
        3 => {
            let DecodeContext::Later(header) = context else {
                return Err(RunStreamCodecError::PayloadForWrongDecodeApi {
                    api: "decode_genesis_record",
                });
            };
            Ok(CanonicalRunRecordPayload::CoveragePlanAccepted {
                plan: decode_coverage_plan(decoder, header)?,
                lease: decode_lease(decoder)?,
            })
        }
        4 => {
            let DecodeContext::Later(header) = context else {
                return Err(RunStreamCodecError::PayloadForWrongDecodeApi {
                    api: "decode_genesis_record",
                });
            };
            Ok(CanonicalRunRecordPayload::FrontierTransition {
                producer_kind: decode_frontier(decoder.u8()?)?,
                previous_frontier_commitment: decoder
                    .digest("frontier_transition.previous_frontier_commitment")?,
                newly_closed: decode_required_frontier(decoder, header.case_universe())?,
                next_frontier_commitment: decoder
                    .digest("frontier_transition.next_frontier_commitment")?,
                semantic_facts: decode_semantic_facts_with_universe(
                    decoder,
                    header.case_universe(),
                )?,
                validation_receipt_hash: decoder
                    .digest("frontier_transition.validation_receipt_hash")?,
                lease: decode_lease(decoder)?,
            })
        }
        5 => Ok(CanonicalRunRecordPayload::Paused {
            reason: decode_pause(decoder.u8()?)?,
            previous_journal_head: decoder.journal_head()?,
            evidence_root: decoder.evidence_root()?,
            lease: decode_lease(decoder)?,
        }),
        6 => Ok(CanonicalRunRecordPayload::Resumed {
            previous_journal_head: decoder.journal_head()?,
            evidence_root: decoder.evidence_root()?,
            lease: decode_lease(decoder)?,
        }),
        7 => Ok(CanonicalRunRecordPayload::Recovered {
            previous_journal_head: decoder.journal_head()?,
            evidence_root: decoder.evidence_root()?,
            lease: decode_lease(decoder)?,
        }),
        8 => Ok(CanonicalRunRecordPayload::TerminalSeal {
            kind: decode_terminal(decoder.u8()?)?,
            journal_head_before_seal: decoder.journal_head()?,
            evidence_root: decoder.evidence_root()?,
            terminal_payload_hash: decoder.terminal_payload_hash()?,
            method_hash: decoder.terminal_method_hash()?,
            lease: decode_lease(decoder)?,
        }),
        value => Err(RunStreamCodecError::InvalidTag {
            field: "payload",
            value,
        }),
    }
}

fn encode_header(
    encoder: &mut Encoder,
    header: &ExploreRunHeader,
) -> Result<(), RunStreamCodecError> {
    let identity = header.identity();
    encoder.digest("header.identity.program_hash", identity.program_hash())?;
    encoder.digest(
        "header.identity.analysis_program_hash",
        identity.analysis_program_hash(),
    )?;
    encoder.digest("header.identity.query_hash", identity.query_hash())?;
    encoder.digest("header.identity.domain_hash", identity.domain_hash())?;
    encoder.digest(
        "header.identity.report_request_hash",
        identity.report_request_hash(),
    )?;
    encoder.digest(
        "header.identity.probe_plan_hash",
        identity.probe_plan_hash(),
    )?;
    encoder.digest(
        "header.identity.evaluator_contract_hash",
        identity.evaluator_contract_hash(),
    )?;
    encoder.digest(
        "header.identity.mechanism_observation_hash",
        identity.mechanism_observation_hash(),
    )?;
    encoder.digest(
        "header.identity.retention_authorization_hash",
        identity.retention_authorization_hash(),
    )?;
    let schemas = identity.schemas();
    encoder.digest("header.schemas.journal_record", schemas.journal_record())?;
    encoder.digest(
        "header.schemas.semantic_evidence",
        schemas.semantic_evidence(),
    )?;
    encoder.digest("header.schemas.snapshot", schemas.snapshot())?;
    encoder.digest("header.schemas.terminal_result", schemas.terminal_result())?;
    encode_universe(encoder, header.case_universe())?;
    encode_obligations(
        encoder,
        "header.required_obligations",
        header.required_obligations(),
    )?;
    encoder.digest("header.nonce", header.nonce().identity())?;
    encoder.digest("header.answer_scope_hash", header.answer_scope_hash())?;
    encoder.digest("header.commitment_hash", header.commitment_hash())?;
    encoder.run_id("header.run_id", header.run_id())?;
    Ok(())
}

fn decode_header(decoder: &mut Decoder<'_>) -> Result<ExploreRunHeader, RunStreamCodecError> {
    let program_hash = decoder.digest("header.identity.program_hash")?;
    let analysis_program_hash = decoder.digest("header.identity.analysis_program_hash")?;
    let query_hash = decoder.digest("header.identity.query_hash")?;
    let domain_hash = decoder.digest("header.identity.domain_hash")?;
    let report_request_hash = decoder.digest("header.identity.report_request_hash")?;
    let probe_plan_hash = decoder.digest("header.identity.probe_plan_hash")?;
    let evaluator_contract_hash = decoder.digest("header.identity.evaluator_contract_hash")?;
    let mechanism_observation_hash =
        decoder.digest("header.identity.mechanism_observation_hash")?;
    let retention_authorization_hash =
        decoder.digest("header.identity.retention_authorization_hash")?;
    let schemas = ExploreRunSchemas::new(
        decoder.digest("header.schemas.journal_record")?,
        decoder.digest("header.schemas.semantic_evidence")?,
        decoder.digest("header.schemas.snapshot")?,
        decoder.digest("header.schemas.terminal_result")?,
    );
    let identity = ExploreRunIdentity::new(
        program_hash,
        analysis_program_hash,
        query_hash,
        domain_hash,
        report_request_hash,
        probe_plan_hash,
        evaluator_contract_hash,
        mechanism_observation_hash,
        retention_authorization_hash,
        schemas,
    );
    let universe = decode_universe(decoder)?;
    let obligations = decode_obligations(decoder, "header.required_obligations")?;
    let nonce = ExploreRunNonce::new(decoder.digest("header.nonce")?)?;
    let recorded_answer_scope = decoder.digest("header.answer_scope_hash")?;
    let recorded_commitment = decoder.digest("header.commitment_hash")?;
    let recorded_run_id = decoder.run_id()?;
    let header = ExploreRunHeader::new(identity, universe, obligations, nonce)?;
    require_digest_equal(
        "header.answer_scope_hash",
        recorded_answer_scope,
        header.answer_scope_hash(),
    )?;
    require_digest_equal(
        "header.commitment_hash",
        recorded_commitment,
        header.commitment_hash(),
    )?;
    if recorded_run_id != header.run_id() {
        return Err(RunStreamCodecError::DerivedIdentityMismatch {
            field: "header.run_id",
        });
    }
    Ok(header)
}

fn encode_universe(
    encoder: &mut Encoder,
    universe: &ExploreCaseUniverse,
) -> Result<(), RunStreamCodecError> {
    encoder.collection_len(
        "case_universe.axes",
        universe.axis_cardinalities().len(),
        CollectionKind::Axes,
    )?;
    for cardinality in universe.axis_cardinalities() {
        encoder.u128(*cardinality)?;
    }
    encoder.u128(universe.case_count())?;
    encoder.digest("case_universe.identity", universe.identity_hash())?;
    Ok(())
}

fn decode_universe(decoder: &mut Decoder<'_>) -> Result<ExploreCaseUniverse, RunStreamCodecError> {
    let axis_count = decoder.collection_len("case_universe.axes", CollectionKind::Axes, 16)?;
    let mut cardinalities = decoder.allocate_vec("case_universe.axes", axis_count)?;
    for _ in 0..axis_count {
        cardinalities.push(decoder.u128()?);
    }
    let recorded_case_count = decoder.u128()?;
    let recorded_identity = decoder.digest("case_universe.identity")?;
    let universe = ExploreCaseUniverse::new(cardinalities)?;
    if recorded_case_count != universe.case_count() {
        return Err(RunStreamCodecError::DerivedScalarMismatch {
            field: "case_universe.case_count",
        });
    }
    require_digest_equal(
        "case_universe.identity",
        recorded_identity,
        universe.identity_hash(),
    )?;
    Ok(universe)
}

fn encode_support(
    encoder: &mut Encoder,
    support: &ExactCaseSupport,
) -> Result<(), RunStreamCodecError> {
    encoder.collection_len(
        "case_support.intervals",
        support.interval_count(),
        CollectionKind::Intervals,
    )?;
    for interval in support.intervals() {
        encoder.u128(interval.start())?;
        encoder.u128(interval.end_exclusive())?;
    }
    encoder.u128(support.case_count())?;
    encoder.digest("case_support.identity", support.identity_hash())?;
    Ok(())
}

fn decode_support(
    decoder: &mut Decoder<'_>,
    universe: &ExploreCaseUniverse,
) -> Result<ExactCaseSupport, RunStreamCodecError> {
    let interval_count =
        decoder.collection_len("case_support.intervals", CollectionKind::Intervals, 32)?;
    let mut intervals = decoder.allocate_vec("case_support.intervals", interval_count)?;
    let mut previous_end = None;
    for index in 0..interval_count {
        let start = decoder.u128()?;
        let end_exclusive = decoder.u128()?;
        if let Some(end) = previous_end {
            if start <= end {
                return Err(RunStreamCodecError::NonCanonicalOrder {
                    field: "case_support.intervals",
                    index,
                });
            }
        }
        previous_end = Some(end_exclusive);
        intervals.push((start, end_exclusive));
    }
    let recorded_case_count = decoder.u128()?;
    let recorded_identity = decoder.digest("case_support.identity")?;
    let support = ExactCaseSupport::new(universe, intervals.iter().copied())?;
    let canonical_intervals = support.intervals();
    if support.interval_count() != intervals.len()
        || canonical_intervals
            .iter()
            .zip(&intervals)
            .any(|(canonical, recorded)| {
                (canonical.start(), canonical.end_exclusive()) != *recorded
            })
    {
        return Err(RunStreamCodecError::NonCanonicalCollection {
            field: "case_support.intervals",
        });
    }
    if recorded_case_count != support.case_count() {
        return Err(RunStreamCodecError::DerivedScalarMismatch {
            field: "case_support.case_count",
        });
    }
    require_digest_equal(
        "case_support.identity",
        recorded_identity,
        support.identity_hash(),
    )?;
    Ok(support)
}

fn encode_obligations(
    encoder: &mut Encoder,
    field: &'static str,
    obligations: &BTreeSet<RequiredObligationId>,
) -> Result<(), RunStreamCodecError> {
    encoder.collection_len(field, obligations.len(), CollectionKind::Obligations)?;
    for obligation in obligations {
        encoder.digest(field, obligation.identity())?;
    }
    Ok(())
}

fn decode_obligations(
    decoder: &mut Decoder<'_>,
    field: &'static str,
) -> Result<Vec<RequiredObligationId>, RunStreamCodecError> {
    let count = decoder.collection_len(field, CollectionKind::Obligations, 32)?;
    let mut obligations = decoder.allocate_vec(field, count)?;
    let mut previous = None;
    for index in 0..count {
        let identity = decoder.digest(field)?;
        if previous.is_some_and(|value| value >= identity) {
            return Err(RunStreamCodecError::NonCanonicalOrder { field, index });
        }
        previous = Some(identity);
        obligations.push(RequiredObligationId::new(identity));
    }
    Ok(obligations)
}

fn encode_frontier(
    encoder: &mut Encoder,
    frontier: &RequiredFrontier,
) -> Result<(), RunStreamCodecError> {
    encode_support(encoder, frontier.open_cases())?;
    encode_obligations(
        encoder,
        "required_frontier.open_obligations",
        frontier.open_obligations(),
    )?;
    encoder.digest("required_frontier.identity", frontier.identity_hash())?;
    Ok(())
}

fn decode_required_frontier(
    decoder: &mut Decoder<'_>,
    universe: &ExploreCaseUniverse,
) -> Result<RequiredFrontier, RunStreamCodecError> {
    let open_cases = decode_support(decoder, universe)?;
    let open_obligations = decode_obligations(decoder, "required_frontier.open_obligations")?;
    let recorded_identity = decoder.digest("required_frontier.identity")?;
    let frontier = RequiredFrontier::new(open_cases, open_obligations)?;
    require_digest_equal(
        "required_frontier.identity",
        recorded_identity,
        frontier.identity_hash(),
    )?;
    Ok(frontier)
}

fn encode_semantic_facts(
    encoder: &mut Encoder,
    facts: &[SemanticEvidenceFact],
) -> Result<(), RunStreamCodecError> {
    encoder.collection_len("semantic_facts", facts.len(), CollectionKind::SemanticFacts)?;
    let mut previous = None;
    for (index, fact) in facts.iter().enumerate() {
        let key = (
            semantic_layer_tag(fact.layer()),
            fact.normalized_content_hash(),
        );
        if previous.is_some_and(|value| value >= key) {
            return Err(RunStreamCodecError::NonCanonicalOrder {
                field: "semantic_facts",
                index,
            });
        }
        previous = Some(key);
        encode_semantic_fact(encoder, fact)?;
    }
    validate_semantic_fact_batch(facts)?;
    Ok(())
}

fn encode_semantic_fact(
    encoder: &mut Encoder,
    fact: &SemanticEvidenceFact,
) -> Result<(), RunStreamCodecError> {
    encoder.u8(semantic_layer_tag(fact.layer()))?;
    encoder.digest(
        "semantic_fact.normalized_content_hash",
        fact.normalized_content_hash(),
    )?;
    match fact.subject() {
        SemanticEvidenceSubject::Cases(support) => {
            encoder.u8(0)?;
            encode_support(encoder, support)?;
        }
        SemanticEvidenceSubject::Obligations(obligations) => {
            encoder.u8(1)?;
            encode_obligations(encoder, "semantic_fact.obligations", obligations)?;
        }
        SemanticEvidenceSubject::Global => encoder.u8(2)?,
    }
    Ok(())
}

fn decode_semantic_facts_with_universe(
    decoder: &mut Decoder<'_>,
    universe: &ExploreCaseUniverse,
) -> Result<Box<[SemanticEvidenceFact]>, RunStreamCodecError> {
    let count = decoder.collection_len("semantic_facts", CollectionKind::SemanticFacts, 34)?;
    let mut facts = decoder.allocate_vec("semantic_facts", count)?;
    let mut previous = None;
    for index in 0..count {
        let fact = decode_semantic_fact(decoder, universe)?;
        let key = (
            semantic_layer_tag(fact.layer()),
            fact.normalized_content_hash(),
        );
        if previous.is_some_and(|value| value >= key) {
            return Err(RunStreamCodecError::NonCanonicalOrder {
                field: "semantic_facts",
                index,
            });
        }
        previous = Some(key);
        facts.push(fact);
    }
    validate_semantic_fact_batch(&facts)?;
    Ok(facts.into_boxed_slice())
}

fn validate_semantic_fact_batch(facts: &[SemanticEvidenceFact]) -> Result<(), RunStreamCodecError> {
    let case_interval_count = facts.iter().try_fold(0_usize, |total, fact| {
        let additional = match (fact.layer(), fact.subject()) {
            (
                SemanticEvidenceLayer::CaseClassification
                | SemanticEvidenceLayer::SemanticTransition,
                SemanticEvidenceSubject::Cases(support),
            ) => support.interval_count(),
            _ => 0,
        };
        total
            .checked_add(additional)
            .ok_or(RunStreamCodecError::LengthOverflow {
                field: "semantic_facts.case_intervals",
            })
    })?;
    if case_interval_count > RUN_STREAM_RECORD_MAX_INTERVALS {
        return Err(RunStreamCodecError::BoundExceeded {
            field: "semantic_facts.case_intervals",
            actual: case_interval_count,
            maximum: RUN_STREAM_RECORD_MAX_INTERVALS,
        });
    }
    let mut case_intervals = Vec::new();
    case_intervals
        .try_reserve_exact(case_interval_count)
        .map_err(|_| RunStreamCodecError::AllocationFailed {
            field: "semantic_facts.case_intervals",
            requested: case_interval_count,
        })?;
    let mut closed_obligations = BTreeSet::new();
    for fact in facts {
        match (fact.layer(), fact.subject()) {
            (
                layer @ (SemanticEvidenceLayer::CaseClassification
                | SemanticEvidenceLayer::SemanticTransition),
                SemanticEvidenceSubject::Cases(support),
            ) => {
                case_intervals.extend(
                    support
                        .intervals()
                        .into_iter()
                        .map(|interval| (layer, interval.start(), interval.end_exclusive())),
                );
            }
            (
                SemanticEvidenceLayer::RepresentativeSelection
                | SemanticEvidenceLayer::ExtremaWitness
                | SemanticEvidenceLayer::MechanismTargetClosure,
                SemanticEvidenceSubject::Obligations(obligations),
            ) => {
                for obligation in obligations {
                    if !closed_obligations.insert(*obligation) {
                        return Err(RunStreamCodecError::ContradictorySemanticFacts);
                    }
                }
            }
            _ => {}
        }
    }
    case_intervals.sort_unstable();
    if case_intervals
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0 && pair[1].1 < pair[0].2)
    {
        return Err(RunStreamCodecError::ContradictorySemanticFacts);
    }
    Ok(())
}

fn decode_semantic_fact(
    decoder: &mut Decoder<'_>,
    universe: &ExploreCaseUniverse,
) -> Result<SemanticEvidenceFact, RunStreamCodecError> {
    let layer = decode_semantic_layer(decoder.u8()?)?;
    let content_hash = decoder.digest("semantic_fact.normalized_content_hash")?;
    let subject = match decoder.u8()? {
        0 => SemanticEvidenceSubject::cases(decode_support(decoder, universe)?),
        1 => SemanticEvidenceSubject::obligations(decode_obligations(
            decoder,
            "semantic_fact.obligations",
        )?)?,
        2 => SemanticEvidenceSubject::global(),
        value => {
            return Err(RunStreamCodecError::InvalidTag {
                field: "semantic_fact.subject",
                value,
            });
        }
    };
    Ok(SemanticEvidenceFact::new(layer, content_hash, subject)?)
}

fn encode_coverage_plan(
    encoder: &mut Encoder,
    plan: &CoveragePlan,
) -> Result<(), RunStreamCodecError> {
    encoder.run_id("coverage_plan.run_id", plan.run_id())?;
    encoder.digest("coverage_plan.proof_set_id", plan.proof_set_id())?;
    encode_support(encoder, plan.certified_closed())?;
    encode_support(encoder, plan.residual_open())?;
    encode_semantic_facts(encoder, plan.semantic_facts())?;
    encoder.digest(
        "coverage_plan.proof_receipt_hash",
        plan.proof_receipt_hash(),
    )?;
    encoder.u64(plan.sharding_epoch().get())?;
    encoder.u64(plan.shard_width().get())?;
    encoder.digest("coverage_plan.identity", plan.identity_hash())?;
    Ok(())
}

fn decode_coverage_plan(
    decoder: &mut Decoder<'_>,
    header: &ExploreRunHeader,
) -> Result<CoveragePlan, RunStreamCodecError> {
    let recorded_run_id = decoder.run_id()?;
    if recorded_run_id != header.run_id() {
        return Err(RunStreamCodecError::RunIdMismatch {
            field: "coverage_plan.run_id",
        });
    }
    let proof_set_id = decoder.digest("coverage_plan.proof_set_id")?;
    let certified_closed = decode_support(decoder, header.case_universe())?;
    let residual_open = decode_support(decoder, header.case_universe())?;
    let semantic_facts = decode_semantic_facts_with_universe(decoder, header.case_universe())?;
    let proof_receipt_hash = decoder.digest("coverage_plan.proof_receipt_hash")?;
    let sharding_epoch = nonzero_u64("coverage_plan.sharding_epoch", decoder.u64()?)?;
    let shard_width = nonzero_u64("coverage_plan.shard_width", decoder.u64()?)?;
    let recorded_identity = decoder.digest("coverage_plan.identity")?;
    let plan = CoveragePlan::new(
        header,
        proof_set_id,
        certified_closed,
        residual_open,
        Vec::from(semantic_facts),
        proof_receipt_hash,
        sharding_epoch,
        shard_width,
    )?;
    require_digest_equal(
        "coverage_plan.identity",
        recorded_identity,
        plan.identity_hash(),
    )?;
    Ok(plan)
}

fn encode_lease(
    encoder: &mut Encoder,
    lease: FencedWriterLease,
) -> Result<(), RunStreamCodecError> {
    encoder.run_id("lease.run_id", lease.run_id())?;
    encoder.u64(lease.generation().get())?;
    encoder.digest("lease.writer_id", lease.writer_id().identity())?;
    encoder.digest("lease.fence_receipt_hash", lease.fence_receipt_hash())?;
    encoder.digest("lease.lease_id", lease.lease_id_hash())?;
    Ok(())
}

fn decode_lease(decoder: &mut Decoder<'_>) -> Result<FencedWriterLease, RunStreamCodecError> {
    let run_id = decoder.run_id()?;
    let generation = nonzero_u64("lease.generation", decoder.u64()?)?;
    let writer_id = ExploreWriterId::new(decoder.digest("lease.writer_id")?);
    let fence_receipt_hash = decoder.digest("lease.fence_receipt_hash")?;
    let expected_lease_id = decoder.digest("lease.lease_id")?;
    Ok(FencedWriterLease::from_recorded_fields(
        run_id,
        generation,
        writer_id,
        fence_receipt_hash,
        expected_lease_id,
    )?)
}

fn encode_event_kind(encoder: &mut Encoder, kind: RunEventKind) -> Result<(), RunStreamCodecError> {
    let (class, variant, detail) = match kind {
        RunEventKind::Control(ControlEventKind::RunOpened) => (0, 0, 0),
        RunEventKind::Control(ControlEventKind::Paused(reason)) => (0, 1, pause_tag(reason)),
        RunEventKind::Control(ControlEventKind::Resumed) => (0, 2, 0),
        RunEventKind::Control(ControlEventKind::Recovered) => (0, 3, 0),
        RunEventKind::Control(ControlEventKind::TerminalSealed(kind)) => (0, 4, terminal_tag(kind)),
        RunEventKind::Discovery(kind) => (1, discovery_tag(kind), 0),
        RunEventKind::Evidence(EvidenceEventKind::CoveragePlanAccepted) => (2, 0, 0),
        RunEventKind::Evidence(EvidenceEventKind::FrontierAdvanced(kind)) => {
            (2, 1, frontier_tag(kind))
        }
        RunEventKind::Evidence(EvidenceEventKind::ObservationAccepted(kind)) => {
            (2, 2, observation_tag(kind))
        }
    };
    encoder.u8(class)?;
    encoder.u8(variant)?;
    encoder.u8(detail)?;
    Ok(())
}

fn decode_event_kind(decoder: &mut Decoder<'_>) -> Result<RunEventKind, RunStreamCodecError> {
    let class = decoder.u8()?;
    let variant = decoder.u8()?;
    let detail = decoder.u8()?;
    match (class, variant) {
        (0, 0) => {
            require_zero("event_kind.run_opened.detail", detail)?;
            Ok(RunEventKind::Control(ControlEventKind::RunOpened))
        }
        (0, 1) => Ok(RunEventKind::Control(ControlEventKind::Paused(
            decode_pause(detail)?,
        ))),
        (0, 2) => {
            require_zero("event_kind.resumed.detail", detail)?;
            Ok(RunEventKind::Control(ControlEventKind::Resumed))
        }
        (0, 3) => {
            require_zero("event_kind.recovered.detail", detail)?;
            Ok(RunEventKind::Control(ControlEventKind::Recovered))
        }
        (0, 4) => Ok(RunEventKind::Control(ControlEventKind::TerminalSealed(
            decode_terminal(detail)?,
        ))),
        (0, value) => Err(RunStreamCodecError::InvalidTag {
            field: "event_kind.control",
            value,
        }),
        (1, value) => {
            require_zero("event_kind.discovery.detail", detail)?;
            Ok(RunEventKind::Discovery(decode_discovery(value)?))
        }
        (2, 0) => {
            require_zero("event_kind.coverage.detail", detail)?;
            Ok(RunEventKind::Evidence(
                EvidenceEventKind::CoveragePlanAccepted,
            ))
        }
        (2, 1) => Ok(RunEventKind::Evidence(EvidenceEventKind::FrontierAdvanced(
            decode_frontier(detail)?,
        ))),
        (2, 2) => Ok(RunEventKind::Evidence(
            EvidenceEventKind::ObservationAccepted(decode_observation(detail)?),
        )),
        (2, value) => Err(RunStreamCodecError::InvalidTag {
            field: "event_kind.evidence",
            value,
        }),
        (value, _) => Err(RunStreamCodecError::InvalidTag {
            field: "event_kind.class",
            value,
        }),
    }
}

fn pause_tag(value: PauseReason) -> u8 {
    match value {
        PauseReason::Explicit => 0,
        PauseReason::TimeLimit => 1,
        PauseReason::Interrupt => 2,
        PauseReason::ResourcePressure => 3,
        PauseReason::StorageLimit => 4,
        PauseReason::ProbeMilestone => 5,
        PauseReason::EvaluationLimit => 6,
        PauseReason::FinalizationPending => 7,
    }
}

fn decode_pause(value: u8) -> Result<PauseReason, RunStreamCodecError> {
    match value {
        0 => Ok(PauseReason::Explicit),
        1 => Ok(PauseReason::TimeLimit),
        2 => Ok(PauseReason::Interrupt),
        3 => Ok(PauseReason::ResourcePressure),
        4 => Ok(PauseReason::StorageLimit),
        5 => Ok(PauseReason::ProbeMilestone),
        6 => Ok(PauseReason::EvaluationLimit),
        7 => Ok(PauseReason::FinalizationPending),
        value => Err(RunStreamCodecError::InvalidTag {
            field: "pause_reason",
            value,
        }),
    }
}

fn discovery_tag(value: DiscoveryEventKind) -> u8 {
    match value {
        DiscoveryEventKind::ProbeDecision => 0,
        DiscoveryEventKind::CandidateDiscovered => 1,
        DiscoveryEventKind::LiftScheduled => 2,
        DiscoveryEventKind::ProbePlanCompleted => 3,
        DiscoveryEventKind::SchedulingHint => 4,
        DiscoveryEventKind::SnapshotPublished => 5,
        DiscoveryEventKind::TerminalResultPublished => 6,
        DiscoveryEventKind::ProbePlanPrepared => 7,
        DiscoveryEventKind::SnapshotUnavailablePublished => 8,
    }
}

fn decode_discovery(value: u8) -> Result<DiscoveryEventKind, RunStreamCodecError> {
    match value {
        0 => Ok(DiscoveryEventKind::ProbeDecision),
        1 => Ok(DiscoveryEventKind::CandidateDiscovered),
        2 => Ok(DiscoveryEventKind::LiftScheduled),
        3 => Ok(DiscoveryEventKind::ProbePlanCompleted),
        4 => Ok(DiscoveryEventKind::SchedulingHint),
        5 => Ok(DiscoveryEventKind::SnapshotPublished),
        6 => Ok(DiscoveryEventKind::TerminalResultPublished),
        7 => Ok(DiscoveryEventKind::ProbePlanPrepared),
        8 => Ok(DiscoveryEventKind::SnapshotUnavailablePublished),
        value => Err(RunStreamCodecError::InvalidTag {
            field: "discovery_kind",
            value,
        }),
    }
}

fn frontier_tag(value: FrontierEvidenceKind) -> u8 {
    match value {
        FrontierEvidenceKind::SingletonClassification => 0,
        FrontierEvidenceKind::CertifiedRegionClassification => 1,
        FrontierEvidenceKind::ExactExhaustion => 2,
        FrontierEvidenceKind::RepresentativeSelectionClosed => 3,
        FrontierEvidenceKind::MechanismTargetClosed => 4,
        FrontierEvidenceKind::BoundedExactBatchClassification => 5,
        FrontierEvidenceKind::ProbeCandidateBatchClassification => 6,
    }
}

fn decode_frontier(value: u8) -> Result<FrontierEvidenceKind, RunStreamCodecError> {
    match value {
        0 => Ok(FrontierEvidenceKind::SingletonClassification),
        1 => Ok(FrontierEvidenceKind::CertifiedRegionClassification),
        2 => Ok(FrontierEvidenceKind::ExactExhaustion),
        3 => Ok(FrontierEvidenceKind::RepresentativeSelectionClosed),
        4 => Ok(FrontierEvidenceKind::MechanismTargetClosed),
        5 => Ok(FrontierEvidenceKind::BoundedExactBatchClassification),
        6 => Ok(FrontierEvidenceKind::ProbeCandidateBatchClassification),
        value => Err(RunStreamCodecError::InvalidTag {
            field: "frontier_evidence_kind",
            value,
        }),
    }
}

fn observation_tag(value: ObservationEvidenceKind) -> u8 {
    match value {
        ObservationEvidenceKind::RepresentativeReplayed => 0,
        ObservationEvidenceKind::MechanismObserved => 1,
        ObservationEvidenceKind::ExtremaWitnessReplayed => 2,
    }
}

fn decode_observation(value: u8) -> Result<ObservationEvidenceKind, RunStreamCodecError> {
    match value {
        0 => Ok(ObservationEvidenceKind::RepresentativeReplayed),
        1 => Ok(ObservationEvidenceKind::MechanismObserved),
        2 => Ok(ObservationEvidenceKind::ExtremaWitnessReplayed),
        value => Err(RunStreamCodecError::InvalidTag {
            field: "observation_evidence_kind",
            value,
        }),
    }
}

fn terminal_tag(value: TerminalSealKind) -> u8 {
    match value {
        TerminalSealKind::Completed => 0,
        TerminalSealKind::Partial => 1,
        TerminalSealKind::Unknown => 2,
        TerminalSealKind::Unsupported => 3,
        TerminalSealKind::Error => 4,
        TerminalSealKind::Cancelled => 5,
    }
}

fn decode_terminal(value: u8) -> Result<TerminalSealKind, RunStreamCodecError> {
    match value {
        0 => Ok(TerminalSealKind::Completed),
        1 => Ok(TerminalSealKind::Partial),
        2 => Ok(TerminalSealKind::Unknown),
        3 => Ok(TerminalSealKind::Unsupported),
        4 => Ok(TerminalSealKind::Error),
        5 => Ok(TerminalSealKind::Cancelled),
        value => Err(RunStreamCodecError::InvalidTag {
            field: "terminal_seal_kind",
            value,
        }),
    }
}

fn semantic_layer_tag(value: SemanticEvidenceLayer) -> u8 {
    match value {
        SemanticEvidenceLayer::CaseClassification => 0,
        SemanticEvidenceLayer::RepresentativeSelection => 1,
        SemanticEvidenceLayer::MechanismObservation => 2,
        SemanticEvidenceLayer::ExtremaWitness => 3,
        SemanticEvidenceLayer::MechanismTargetClosure => 4,
        SemanticEvidenceLayer::AnswerAggregation => 5,
        SemanticEvidenceLayer::SemanticTransition => 6,
    }
}

fn decode_semantic_layer(value: u8) -> Result<SemanticEvidenceLayer, RunStreamCodecError> {
    match value {
        0 => Ok(SemanticEvidenceLayer::CaseClassification),
        1 => Ok(SemanticEvidenceLayer::RepresentativeSelection),
        2 => Ok(SemanticEvidenceLayer::MechanismObservation),
        3 => Ok(SemanticEvidenceLayer::ExtremaWitness),
        4 => Ok(SemanticEvidenceLayer::MechanismTargetClosure),
        5 => Ok(SemanticEvidenceLayer::AnswerAggregation),
        6 => Ok(SemanticEvidenceLayer::SemanticTransition),
        value => Err(RunStreamCodecError::InvalidTag {
            field: "semantic_evidence_layer",
            value,
        }),
    }
}

fn require_zero(field: &'static str, value: u8) -> Result<(), RunStreamCodecError> {
    if value != 0 {
        return Err(RunStreamCodecError::ReservedByteNonzero { field, value });
    }
    Ok(())
}

fn nonzero_u64(field: &'static str, value: u64) -> Result<NonZeroU64, RunStreamCodecError> {
    NonZeroU64::new(value).ok_or(RunStreamCodecError::ZeroNotAllowed { field })
}

fn require_digest_equal(
    field: &'static str,
    recorded: CanonicalDigest,
    derived: CanonicalDigest,
) -> Result<(), RunStreamCodecError> {
    if recorded != derived {
        return Err(RunStreamCodecError::DerivedIdentityMismatch { field });
    }
    Ok(())
}

struct Encoder {
    bytes: Vec<u8>,
    budget: RecordBudget,
}

impl Encoder {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            budget: RecordBudget::default(),
        }
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), RunStreamCodecError> {
        let next = self.bytes.len().checked_add(value.len()).ok_or(
            RunStreamCodecError::LengthOverflow {
                field: "record_bytes",
            },
        )?;
        if next > RUN_STREAM_RECORD_MAX_BYTES {
            return Err(RunStreamCodecError::RecordTooLarge {
                actual: next,
                maximum: RUN_STREAM_RECORD_MAX_BYTES,
            });
        }
        self.bytes
            .try_reserve(value.len())
            .map_err(|_| RunStreamCodecError::AllocationFailed {
                field: "record_bytes",
                requested: next,
            })?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), RunStreamCodecError> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), RunStreamCodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), RunStreamCodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), RunStreamCodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<(), RunStreamCodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn patch_u32(&mut self, offset: usize, value: u32) -> Result<(), RunStreamCodecError> {
        let end = offset
            .checked_add(4)
            .ok_or(RunStreamCodecError::LengthOverflow {
                field: "payload_length_offset",
            })?;
        let destination =
            self.bytes
                .get_mut(offset..end)
                .ok_or(RunStreamCodecError::InternalInvariant {
                    field: "payload_length_offset",
                })?;
        destination.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn collection_len(
        &mut self,
        field: &'static str,
        count: usize,
        kind: CollectionKind,
    ) -> Result<(), RunStreamCodecError> {
        self.budget.charge(field, kind, count)?;
        let count =
            u32::try_from(count).map_err(|_| RunStreamCodecError::LengthOverflow { field })?;
        self.u32(count)
    }

    fn digest(
        &mut self,
        field: &'static str,
        value: CanonicalDigest,
    ) -> Result<(), RunStreamCodecError> {
        self.hex_digest(field, &value.to_lowercase_hex())
    }

    fn run_id(
        &mut self,
        field: &'static str,
        value: ExploreRunId,
    ) -> Result<(), RunStreamCodecError> {
        self.hex_digest(field, &value.to_lowercase_hex())
    }

    fn journal_head(
        &mut self,
        field: &'static str,
        value: JournalHead,
    ) -> Result<(), RunStreamCodecError> {
        self.hex_digest(field, &value.to_lowercase_hex())
    }

    fn evidence_root(
        &mut self,
        field: &'static str,
        value: EvidenceRoot,
    ) -> Result<(), RunStreamCodecError> {
        self.hex_digest(field, &value.to_lowercase_hex())
    }

    fn terminal_payload_hash(
        &mut self,
        value: TerminalPayloadHash,
    ) -> Result<(), RunStreamCodecError> {
        self.hex_digest(
            "terminal_seal.terminal_payload_hash",
            &value.to_lowercase_hex(),
        )
    }

    fn terminal_method_hash(
        &mut self,
        value: TerminalMethodHash,
    ) -> Result<(), RunStreamCodecError> {
        self.hex_digest("terminal_seal.method_hash", &value.to_lowercase_hex())
    }

    fn hex_digest(&mut self, field: &'static str, value: &str) -> Result<(), RunStreamCodecError> {
        let raw = decode_lowercase_hex_32(field, value)?;
        self.bytes(&raw)
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    budget: RecordBudget,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            budget: RecordBudget::default(),
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RunStreamCodecError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(RunStreamCodecError::LengthOverflow {
                field: "decoder.offset",
            })?;
        let Some(value) = self.bytes.get(self.offset..end) else {
            return Err(RunStreamCodecError::Truncated {
                offset: self.offset,
                needed: count,
                remaining: self.remaining(),
            });
        };
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RunStreamCodecError> {
        let mut result = [0_u8; N];
        result.copy_from_slice(self.take(N)?);
        Ok(result)
    }

    fn u8(&mut self) -> Result<u8, RunStreamCodecError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, RunStreamCodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RunStreamCodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, RunStreamCodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn u128(&mut self) -> Result<u128, RunStreamCodecError> {
        Ok(u128::from_le_bytes(self.array()?))
    }

    fn collection_len(
        &mut self,
        field: &'static str,
        kind: CollectionKind,
        minimum_item_bytes: usize,
    ) -> Result<usize, RunStreamCodecError> {
        let count = self.u32()? as usize;
        self.budget.charge(field, kind, count)?;
        let minimum_bytes = count
            .checked_mul(minimum_item_bytes)
            .ok_or(RunStreamCodecError::LengthOverflow { field })?;
        if minimum_bytes > self.remaining() {
            return Err(RunStreamCodecError::TruncatedCollection {
                field,
                count,
                minimum_item_bytes,
                remaining: self.remaining(),
            });
        }
        Ok(count)
    }

    fn allocate_vec<T>(
        &self,
        field: &'static str,
        capacity: usize,
    ) -> Result<Vec<T>, RunStreamCodecError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| RunStreamCodecError::AllocationFailed {
                field,
                requested: capacity,
            })?;
        Ok(values)
    }

    fn digest(&mut self, field: &'static str) -> Result<CanonicalDigest, RunStreamCodecError> {
        let raw = self.array::<32>()?;
        let text = lowercase_hex_bytes(&raw);
        let text = std::str::from_utf8(&text)
            .map_err(|_| RunStreamCodecError::InternalDigestEncoding { field })?;
        Ok(CanonicalDigest::from_lowercase_sha256(field, text)?)
    }

    fn run_id(&mut self) -> Result<ExploreRunId, RunStreamCodecError> {
        let raw = self.array::<32>()?;
        let text = lowercase_hex_bytes(&raw);
        let text = std::str::from_utf8(&text)
            .map_err(|_| RunStreamCodecError::InternalDigestEncoding { field: "run_id" })?;
        Ok(ExploreRunId::from_lowercase_sha256(text)?)
    }

    fn journal_head(&mut self) -> Result<JournalHead, RunStreamCodecError> {
        let raw = self.array::<32>()?;
        let text = lowercase_hex_bytes(&raw);
        let text = std::str::from_utf8(&text).map_err(|_| {
            RunStreamCodecError::InternalDigestEncoding {
                field: "journal_head",
            }
        })?;
        Ok(JournalHead::from_lowercase_sha256(text)?)
    }

    fn evidence_root(&mut self) -> Result<EvidenceRoot, RunStreamCodecError> {
        let raw = self.array::<32>()?;
        let text = lowercase_hex_bytes(&raw);
        let text = std::str::from_utf8(&text).map_err(|_| {
            RunStreamCodecError::InternalDigestEncoding {
                field: "evidence_root",
            }
        })?;
        Ok(EvidenceRoot::from_lowercase_sha256(text)?)
    }

    fn terminal_payload_hash(&mut self) -> Result<TerminalPayloadHash, RunStreamCodecError> {
        let raw = self.array::<32>()?;
        let text = lowercase_hex_bytes(&raw);
        let text = std::str::from_utf8(&text).map_err(|_| {
            RunStreamCodecError::InternalDigestEncoding {
                field: "terminal_payload_hash",
            }
        })?;
        Ok(TerminalPayloadHash::from_lowercase_sha256(text)?)
    }

    fn terminal_method_hash(&mut self) -> Result<TerminalMethodHash, RunStreamCodecError> {
        let raw = self.array::<32>()?;
        let text = lowercase_hex_bytes(&raw);
        let text = std::str::from_utf8(&text).map_err(|_| {
            RunStreamCodecError::InternalDigestEncoding {
                field: "terminal_method_hash",
            }
        })?;
        Ok(TerminalMethodHash::from_lowercase_sha256(text)?)
    }
}

#[derive(Clone, Copy)]
enum CollectionKind {
    Axes,
    Intervals,
    Obligations,
    SemanticFacts,
}

#[derive(Default)]
struct RecordBudget {
    axes: usize,
    intervals: usize,
    obligations: usize,
    semantic_facts: usize,
}

impl RecordBudget {
    fn charge(
        &mut self,
        field: &'static str,
        kind: CollectionKind,
        count: usize,
    ) -> Result<(), RunStreamCodecError> {
        let (used, maximum) = match kind {
            CollectionKind::Axes => (&mut self.axes, RUN_STREAM_RECORD_MAX_AXES),
            CollectionKind::Intervals => (&mut self.intervals, RUN_STREAM_RECORD_MAX_INTERVALS),
            CollectionKind::Obligations => {
                (&mut self.obligations, RUN_STREAM_RECORD_MAX_OBLIGATIONS)
            }
            CollectionKind::SemanticFacts => (
                &mut self.semantic_facts,
                RUN_STREAM_RECORD_MAX_SEMANTIC_FACTS,
            ),
        };
        let actual = (*used)
            .checked_add(count)
            .ok_or(RunStreamCodecError::BoundExceeded {
                field,
                actual: usize::MAX,
                maximum,
            })?;
        if actual > maximum {
            return Err(RunStreamCodecError::BoundExceeded {
                field,
                actual,
                maximum,
            });
        }
        *used = actual;
        Ok(())
    }
}

fn decode_lowercase_hex_32(
    field: &'static str,
    value: &str,
) -> Result<[u8; 32], RunStreamCodecError> {
    if value.len() != 64 {
        return Err(RunStreamCodecError::InternalDigestEncoding { field });
    }
    let mut raw = [0_u8; 32];
    for (index, byte) in raw.iter_mut().enumerate() {
        let high = decode_lowercase_hex_nibble(value.as_bytes()[index * 2]);
        let low = decode_lowercase_hex_nibble(value.as_bytes()[index * 2 + 1]);
        let (Some(high), Some(low)) = (high, low) else {
            return Err(RunStreamCodecError::InternalDigestEncoding { field });
        };
        *byte = (high << 4) | low;
    }
    Ok(raw)
}

fn decode_lowercase_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn lowercase_hex_bytes(bytes: &[u8; 32]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = [0_u8; 64];
    for (index, byte) in bytes.iter().enumerate() {
        result[index * 2] = HEX[(byte >> 4) as usize];
        result[index * 2 + 1] = HEX[(byte & 0x0f) as usize];
    }
    result
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunStreamCodecError {
    RecordTooLarge {
        actual: usize,
        maximum: usize,
    },
    InvalidMagic,
    InvalidDomain,
    UnsupportedVersion {
        version: u16,
    },
    UnsupportedFlags {
        flags: u16,
    },
    Truncated {
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    TruncatedCollection {
        field: &'static str,
        count: usize,
        minimum_item_bytes: usize,
        remaining: usize,
    },
    PayloadLengthMismatch {
        declared: u32,
        remaining: usize,
    },
    TrailingBytes {
        field: &'static str,
        remaining: usize,
    },
    LengthOverflow {
        field: &'static str,
    },
    BoundExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    AllocationFailed {
        field: &'static str,
        requested: usize,
    },
    InvalidTag {
        field: &'static str,
        value: u8,
    },
    ReservedByteNonzero {
        field: &'static str,
        value: u8,
    },
    ZeroNotAllowed {
        field: &'static str,
    },
    InvalidGenesisLeaseGeneration {
        actual: u64,
    },
    NonCanonicalOrder {
        field: &'static str,
        index: usize,
    },
    NonCanonicalCollection {
        field: &'static str,
    },
    ContradictorySemanticFacts,
    DerivedIdentityMismatch {
        field: &'static str,
    },
    DerivedScalarMismatch {
        field: &'static str,
    },
    RunIdMismatch {
        field: &'static str,
    },
    SequenceForWrongDecodeApi {
        api: &'static str,
        sequence: u64,
    },
    PayloadForWrongDecodeApi {
        api: &'static str,
    },
    EnvelopeMismatch,
    NonCanonicalEncoding,
    InternalDigestEncoding {
        field: &'static str,
    },
    InternalInvariant {
        field: &'static str,
    },
    RunStream(ExploreRunStreamError),
}

impl fmt::Display for RunStreamCodecError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordTooLarge { actual, maximum } => write!(
                out,
                "Explore run record is {actual} bytes; codec limit is {maximum} bytes"
            ),
            Self::InvalidMagic => write!(out, "Explore run record magic is invalid"),
            Self::InvalidDomain => write!(out, "Explore run record domain is invalid"),
            Self::UnsupportedVersion { version } => {
                write!(out, "Explore run record codec version {version} is unsupported")
            }
            Self::UnsupportedFlags { flags } => {
                write!(out, "Explore run record has unsupported flags 0x{flags:04x}")
            }
            Self::Truncated {
                offset,
                needed,
                remaining,
            } => write!(
                out,
                "Explore run record is truncated at byte {offset}: needed {needed}, had {remaining}"
            ),
            Self::TruncatedCollection {
                field,
                count,
                minimum_item_bytes,
                remaining,
            } => write!(
                out,
                "Explore run record {field} declares {count} items requiring at least \
                 {minimum_item_bytes} bytes each, with only {remaining} bytes remaining"
            ),
            Self::PayloadLengthMismatch {
                declared,
                remaining,
            } => write!(
                out,
                "Explore run record declares {declared} payload bytes but has {remaining}"
            ),
            Self::TrailingBytes { field, remaining } => write!(
                out,
                "Explore run record {field} has {remaining} trailing bytes"
            ),
            Self::LengthOverflow { field } => {
                write!(out, "Explore run record {field} length overflowed")
            }
            Self::BoundExceeded {
                field,
                actual,
                maximum,
            } => write!(
                out,
                "Explore run record {field} raises its record-wide count to {actual}; limit is {maximum}"
            ),
            Self::AllocationFailed { field, requested } => write!(
                out,
                "Explore run record could not allocate {requested} slots/bytes for {field}"
            ),
            Self::InvalidTag { field, value } => {
                write!(out, "Explore run record {field} tag {value} is invalid")
            }
            Self::ReservedByteNonzero { field, value } => write!(
                out,
                "Explore run record reserved {field} byte is {value}, expected zero"
            ),
            Self::ZeroNotAllowed { field } => {
                write!(out, "Explore run record {field} must be nonzero")
            }
            Self::InvalidGenesisLeaseGeneration { actual } => write!(
                out,
                "Explore RunOpened lease generation is {actual}, expected 1"
            ),
            Self::NonCanonicalOrder { field, index } => write!(
                out,
                "Explore run record {field} is not strictly canonical at item {index}"
            ),
            Self::NonCanonicalCollection { field } => {
                write!(out, "Explore run record {field} is not canonically normalized")
            }
            Self::ContradictorySemanticFacts => write!(
                out,
                "Explore run record contains overlapping or contradictory semantic facts"
            ),
            Self::DerivedIdentityMismatch { field } => write!(
                out,
                "Explore run record {field} does not match its reconstructed identity"
            ),
            Self::DerivedScalarMismatch { field } => write!(
                out,
                "Explore run record {field} does not match its reconstructed value"
            ),
            Self::RunIdMismatch { field } => {
                write!(out, "Explore run record {field} belongs to another run")
            }
            Self::SequenceForWrongDecodeApi { api, sequence } => write!(
                out,
                "Explore run sequence {sequence} is invalid for {api}"
            ),
            Self::PayloadForWrongDecodeApi { api } => {
                write!(out, "Explore run payload variant is invalid for {api}")
            }
            Self::EnvelopeMismatch => write!(
                out,
                "Explore run envelope does not reproduce the supplied committed event"
            ),
            Self::NonCanonicalEncoding => {
                write!(out, "Explore run record has a noncanonical binary spelling")
            }
            Self::InternalDigestEncoding { field } => write!(
                out,
                "Explore run codec received a noncanonical in-memory digest for {field}"
            ),
            Self::InternalInvariant { field } => {
                write!(out, "Explore run codec internal invariant failed for {field}")
            }
            Self::RunStream(error) => fmt::Display::fmt(error, out),
        }
    }
}

impl Error for RunStreamCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RunStream(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ExploreRunStreamError> for RunStreamCodecError {
    fn from(value: ExploreRunStreamError) -> Self {
        Self::RunStream(value)
    }
}
