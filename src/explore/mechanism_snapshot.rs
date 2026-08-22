//! Bounded canonical JSON-lines projection of a running mechanism stream.
//!
//! This is intentionally not the terminal mechanism artifact. It exposes only
//! cursor-bound population, signature-count and requested-bin summaries,
//! including the honest zero-evidence state immediately after probes.
//! Signature definitions, case IDs, retained examples and incidence DAGs
//! remain private to the reducer.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;

use sha2::{Digest, Sha256};

use super::mechanism::{MechanismCount, MechanismEvidenceStatus};
use super::mechanism_stream::{
    MechanismCheckpointBinFieldV1, MechanismCheckpointCountV1, MechanismCheckpointSummaryV1,
};
use super::run_stream::{
    CanonicalDigest, EvidenceRoot, ExploreRunCursor, ExploreRunId, JournalHead, RunLifecycle,
};

pub(crate) const MECHANISM_OBSERVABLE_CHECKPOINT_BLOB_KIND_V1: &str =
    "mechanism-observable-checkpoint-v1";
pub(crate) const MECHANISM_OBSERVABLE_CHECKPOINT_SCHEMA_V1: &str =
    "futuruna.explore.mechanism-checkpoint.v1";
pub(crate) const MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_BLOB_KIND_V1: &str =
    "mechanism-observable-checkpoint-unavailable-v1";
pub(crate) const MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_SCHEMA_V1: &str =
    "futuruna.explore.mechanism-checkpoint-unavailable.v1";

pub(crate) const MECHANISM_OBSERVABLE_CHECKPOINT_JSON_BYTE_LIMIT_V1: usize = 64 * 1024 * 1024;
pub(crate) const MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_JSON_BYTE_LIMIT_V1: usize = 4 * 1024;

const MECHANISM_CHECKPOINT_MAX_FIELD_COUNT_V1: usize = 256;
const MECHANISM_CHECKPOINT_MAX_BINS_PER_FIELD_V1: usize = 65_536;
const MECHANISM_CHECKPOINT_MAX_TOTAL_BINS_V1: usize = 262_144;
const MECHANISM_CHECKPOINT_MAX_FIELD_NAME_BYTES_V1: usize = 4_096;

/// Identity contribution for the canonical checkpoint syntax, semantics and
/// every publication ceiling. A stream identity which authorizes this view
/// must bind this digest before observations begin.
pub(crate) fn mechanism_observable_checkpoint_contract_digest_v1() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"futuruna.explore.mechanism-observable-checkpoint-contract.v1");
    for segment in [
        MECHANISM_OBSERVABLE_CHECKPOINT_BLOB_KIND_V1.as_bytes(),
        MECHANISM_OBSERVABLE_CHECKPOINT_SCHEMA_V1.as_bytes(),
        MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_BLOB_KIND_V1.as_bytes(),
        MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_SCHEMA_V1.as_bytes(),
        b"cursor-bound-count-only-summary-v1".as_slice(),
        b"exact-lower-bound-unknown-certainty-v1".as_slice(),
        b"case-closure-biconditional-target-certainty-v1".as_slice(),
        b"distinct-mechanism-counts-are-non-additive-v1".as_slice(),
        b"no-signatures-cases-examples-or-incidence-dags-v1".as_slice(),
    ] {
        hasher.update((segment.len() as u64).to_le_bytes());
        hasher.update(segment);
    }
    for (name, limit) in [
        (
            b"checkpoint-json-bytes".as_slice(),
            MECHANISM_OBSERVABLE_CHECKPOINT_JSON_BYTE_LIMIT_V1,
        ),
        (
            b"checkpoint-unavailable-json-bytes".as_slice(),
            MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_JSON_BYTE_LIMIT_V1,
        ),
        (
            b"checkpoint-field-count".as_slice(),
            MECHANISM_CHECKPOINT_MAX_FIELD_COUNT_V1,
        ),
        (
            b"checkpoint-bins-per-field".as_slice(),
            MECHANISM_CHECKPOINT_MAX_BINS_PER_FIELD_V1,
        ),
        (
            b"checkpoint-total-bins".as_slice(),
            MECHANISM_CHECKPOINT_MAX_TOTAL_BINS_V1,
        ),
        (
            b"checkpoint-field-name-bytes".as_slice(),
            MECHANISM_CHECKPOINT_MAX_FIELD_NAME_BYTES_V1,
        ),
    ] {
        hasher.update((name.len() as u64).to_le_bytes());
        hasher.update(name);
        hasher.update((limit as u128).to_le_bytes());
    }
    hasher.finalize().into()
}

/// Cursor and frontier facts attached to one mechanism checkpoint.
///
/// Construction requires the running pre-publication cursor and completed
/// probe milestone. A paused/sealed cursor or pre-probe mechanism view fails
/// before any bytes are rendered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MechanismObservableCheckpointMetadataV1 {
    schema_digest: CanonicalDigest,
    run_id: ExploreRunId,
    sequence: u64,
    journal_head: JournalHead,
    evidence_root: EvidenceRoot,
    last_coverage_epoch: Option<NonZeroU64>,
    probe_milestone_complete: bool,
    universe_case_count: u128,
    closed_case_count: u128,
}

impl MechanismObservableCheckpointMetadataV1 {
    pub(crate) fn from_running_cursor(
        schema_digest: CanonicalDigest,
        cursor: ExploreRunCursor,
        probe_milestone_complete: bool,
        universe_case_count: u128,
        closed_case_count: u128,
    ) -> Result<Self, MechanismCheckpointRenderError> {
        if cursor.lifecycle() != RunLifecycle::Running {
            return Err(MechanismCheckpointRenderError::invalid(
                "mechanism checkpoint requires a running pre-publication cursor",
            ));
        }
        if !probe_milestone_complete {
            return Err(MechanismCheckpointRenderError::invalid(
                "mechanism checkpoint publication requires the completed probe milestone",
            ));
        }
        if closed_case_count > universe_case_count {
            return Err(MechanismCheckpointRenderError::invalid(
                "mechanism checkpoint closed cases exceed the case universe",
            ));
        }
        Ok(Self {
            schema_digest,
            run_id: cursor.run_id(),
            sequence: cursor.sequence(),
            journal_head: cursor.journal_head(),
            evidence_root: cursor.evidence_root(),
            last_coverage_epoch: cursor.last_coverage_epoch(),
            probe_milestone_complete,
            universe_case_count,
            closed_case_count,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MechanismCheckpointRenderError {
    message: Box<str>,
    capacity_limit: bool,
}

impl MechanismCheckpointRenderError {
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

impl fmt::Display for MechanismCheckpointRenderError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.message)
    }
}

impl Error for MechanismCheckpointRenderError {}

/// Render one complete cursor-bearing mechanism checkpoint plus exactly one
/// trailing LF. Crossing any fixed limit drops the private buffer and returns
/// an error; a prefix can never become an artifact.
pub(crate) fn render_mechanism_observable_checkpoint_json_line_v1(
    metadata: &MechanismObservableCheckpointMetadataV1,
    summary: &MechanismCheckpointSummaryV1,
) -> Result<Vec<u8>, MechanismCheckpointRenderError> {
    validate_checkpoint(metadata, summary)?;
    let mut writer =
        CanonicalJsonWriter::with_max_bytes(MECHANISM_OBSERVABLE_CHECKPOINT_JSON_BYTE_LIMIT_V1);
    writer.raw(b"{")?;
    writer.member_string("schema", MECHANISM_OBSERVABLE_CHECKPOINT_SCHEMA_V1)?;
    writer.raw(b",")?;
    writer.member_u64("schema_version", 1)?;
    writer.raw(b",")?;
    writer.member_string("schema_digest", &metadata.schema_digest.to_lowercase_hex())?;
    writer.raw(b",\"run\":{")?;
    writer.member_string("run_id", &metadata.run_id.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_decimal("sequence", metadata.sequence)?;
    writer.raw(b",")?;
    writer.member_string("journal_head", &metadata.journal_head.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string("evidence_root", &metadata.evidence_root.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string("lifecycle", "running")?;
    writer.raw(b",")?;
    writer.member_bool(
        "probe_milestone_complete",
        metadata.probe_milestone_complete,
    )?;
    writer.raw(b",")?;
    writer.member_optional_decimal(
        "last_coverage_epoch",
        metadata.last_coverage_epoch.map(NonZeroU64::get),
    )?;
    writer.raw(b"},\"progress\":{")?;
    writer.member_decimal("universe_case_count", metadata.universe_case_count)?;
    writer.raw(b",")?;
    writer.member_decimal("closed_case_count", metadata.closed_case_count)?;
    writer.raw(b",")?;
    writer.member_decimal(
        "open_case_count",
        metadata.universe_case_count - metadata.closed_case_count,
    )?;
    writer.raw(b"},\"mechanism\":{")?;
    writer.member_string("target", "matching_configurations")?;
    writer.raw(b",")?;
    writer.member_string(
        "checked_request_hash",
        &lowercase_hex(&summary.checked_request_hash),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "observation_spec_hash",
        &lowercase_hex(&summary.observation_spec_hash),
    )?;
    writer.raw(b",")?;
    writer.member_string("status", evidence_status_name(summary.status))?;
    writer.raw(b",\"target_cases\":")?;
    write_mechanism_count(&mut writer, mechanism_count(summary.target_cases))?;
    writer.raw(b",")?;
    writer.member_decimal("traced_cases", summary.traced_cases)?;
    writer.raw(b",\"known_target_untraced\":{")?;
    writer.member_decimal("total", summary.known_target_untraced.total)?;
    writer.raw(b",")?;
    writer.member_decimal("pending", summary.known_target_untraced.pending)?;
    writer.raw(b",")?;
    writer.member_decimal(
        "replay_unavailable",
        summary.known_target_untraced.replay_unavailable,
    )?;
    writer.raw(b",")?;
    writer.member_decimal(
        "observation_unsupported",
        summary.known_target_untraced.observation_unsupported,
    )?;
    writer.raw(b"},\"mechanism_signatures\":")?;
    write_mechanism_count(&mut writer, summary.mechanism_signatures)?;
    writer.raw(b",\"bin_fields\":[")?;
    for (index, field) in summary.bin_fields.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        write_bin_field(&mut writer, field)?;
    }
    writer.raw(b"]}}\n")?;
    Ok(writer.finish())
}

/// Bounded alternative emitted only when a validated full checkpoint crosses
/// its canonical byte ceiling. It authenticates the exact cursor and request,
/// but never exposes a truncated field or bin list.
pub(crate) fn render_mechanism_observable_checkpoint_unavailable_json_line_v1(
    metadata: &MechanismObservableCheckpointMetadataV1,
    summary: &MechanismCheckpointSummaryV1,
) -> Result<Vec<u8>, MechanismCheckpointRenderError> {
    validate_checkpoint(metadata, summary)?;
    let mut writer = CanonicalJsonWriter::with_max_bytes(
        MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_JSON_BYTE_LIMIT_V1,
    );
    writer.raw(b"{")?;
    writer.member_string(
        "schema",
        MECHANISM_OBSERVABLE_CHECKPOINT_UNAVAILABLE_SCHEMA_V1,
    )?;
    writer.raw(b",")?;
    writer.member_u64("schema_version", 1)?;
    writer.raw(b",")?;
    writer.member_string("schema_digest", &metadata.schema_digest.to_lowercase_hex())?;
    writer.raw(b",\"run\":{")?;
    writer.member_string("run_id", &metadata.run_id.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_decimal("sequence", metadata.sequence)?;
    writer.raw(b",")?;
    writer.member_string("journal_head", &metadata.journal_head.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string("evidence_root", &metadata.evidence_root.to_lowercase_hex())?;
    writer.raw(b",")?;
    writer.member_string("lifecycle", "running")?;
    writer.raw(b",")?;
    writer.member_optional_decimal(
        "last_coverage_epoch",
        metadata.last_coverage_epoch.map(NonZeroU64::get),
    )?;
    writer.raw(b"},\"mechanism_checkpoint\":{")?;
    writer.member_string("status", "unavailable")?;
    writer.raw(b",\"reason\":{")?;
    writer.member_string("kind", "capacity")?;
    writer.raw(b"},")?;
    writer.member_string(
        "checked_request_hash",
        &lowercase_hex(&summary.checked_request_hash),
    )?;
    writer.raw(b",")?;
    writer.member_string(
        "observation_spec_hash",
        &lowercase_hex(&summary.observation_spec_hash),
    )?;
    writer.raw(b"},\"progress\":{")?;
    writer.member_bool(
        "probe_milestone_complete",
        metadata.probe_milestone_complete,
    )?;
    writer.raw(b",")?;
    writer.member_decimal("universe_case_count", metadata.universe_case_count)?;
    writer.raw(b",")?;
    writer.member_decimal("closed_case_count", metadata.closed_case_count)?;
    writer.raw(b"}}\n")?;
    Ok(writer.finish())
}

fn validate_checkpoint(
    metadata: &MechanismObservableCheckpointMetadataV1,
    summary: &MechanismCheckpointSummaryV1,
) -> Result<(), MechanismCheckpointRenderError> {
    if !metadata.probe_milestone_complete {
        return Err(MechanismCheckpointRenderError::invalid(
            "mechanism checkpoint publication requires the completed probe milestone",
        ));
    }
    if metadata.closed_case_count > metadata.universe_case_count {
        return Err(MechanismCheckpointRenderError::invalid(
            "mechanism checkpoint closed cases exceed the case universe",
        ));
    }
    let classification_closed = metadata.closed_case_count == metadata.universe_case_count;
    let target_exact = matches!(summary.target_cases, MechanismCount::Exact(_));
    if classification_closed != target_exact {
        return Err(MechanismCheckpointRenderError::invalid(
            "mechanism target certainty must be exact if and only if case classification is closed",
        ));
    }

    let target_count = summary.target_cases.value();
    if target_count > metadata.closed_case_count {
        return Err(MechanismCheckpointRenderError::invalid(
            "known mechanism target cases exceed the durable closed-case frontier",
        ));
    }
    let permanent_untraced = checked_add(
        summary.known_target_untraced.replay_unavailable,
        summary.known_target_untraced.observation_unsupported,
        "permanently-untraced mechanism checkpoint cases",
    )?;
    let untraced = checked_add(
        summary.known_target_untraced.pending,
        permanent_untraced,
        "known-target untraced mechanism checkpoint cases",
    )?;
    if untraced != summary.known_target_untraced.total {
        return Err(MechanismCheckpointRenderError::invalid(
            "mechanism checkpoint untraced reasons do not conserve their total",
        ));
    }
    if checked_add(
        summary.traced_cases,
        untraced,
        "mechanism checkpoint target cases",
    )? != target_count
    {
        return Err(MechanismCheckpointRenderError::invalid(
            "mechanism checkpoint traced and untraced cases do not conserve the target",
        ));
    }
    match (summary.status, summary.target_cases) {
        (MechanismEvidenceStatus::ScopeOpen, MechanismCount::LowerBound(_)) => {}
        (
            MechanismEvidenceStatus::IncidenceOpen | MechanismEvidenceStatus::MatchingClosed,
            MechanismCount::Exact(_),
        ) => {}
        _ => {
            return Err(MechanismCheckpointRenderError::invalid(
                "mechanism checkpoint status disagrees with target-count certainty",
            ));
        }
    }
    match summary.status {
        MechanismEvidenceStatus::ScopeOpen => {}
        MechanismEvidenceStatus::IncidenceOpen if untraced > 0 => {}
        MechanismEvidenceStatus::MatchingClosed if untraced == 0 => {}
        MechanismEvidenceStatus::IncidenceOpen | MechanismEvidenceStatus::MatchingClosed => {
            return Err(MechanismCheckpointRenderError::invalid(
                "mechanism checkpoint status disagrees with remaining target incidence",
            ));
        }
    }

    let confirmed_signatures = summary.mechanism_signatures.confirmed_lower_bound();
    if confirmed_signatures > summary.traced_cases {
        return Err(MechanismCheckpointRenderError::invalid(
            "confirmed mechanism signatures exceed traced cases",
        ));
    }
    let expected_signatures = expected_checkpoint_count(summary.status, 0, confirmed_signatures);
    if summary.mechanism_signatures != expected_signatures {
        return Err(MechanismCheckpointRenderError::invalid(
            "mechanism signature count uses certainty unsupported by current closure",
        ));
    }

    if summary.bin_fields.len() > MECHANISM_CHECKPOINT_MAX_FIELD_COUNT_V1 {
        return Err(MechanismCheckpointRenderError::limit(format!(
            "mechanism checkpoint field count exceeds {}",
            MECHANISM_CHECKPOINT_MAX_FIELD_COUNT_V1
        )));
    }
    let mut names = BTreeSet::new();
    let mut total_bins = 0_usize;
    for field in summary.bin_fields.iter() {
        if field.name.is_empty() || field.name.len() > MECHANISM_CHECKPOINT_MAX_FIELD_NAME_BYTES_V1
        {
            return Err(MechanismCheckpointRenderError::invalid(
                "mechanism checkpoint field name is empty or exceeds its fixed byte limit",
            ));
        }
        if !names.insert(field.name.as_ref()) {
            return Err(MechanismCheckpointRenderError::invalid(
                "mechanism checkpoint field names are not unique",
            ));
        }
        if field.bins.is_empty() || field.bins.len() > MECHANISM_CHECKPOINT_MAX_BINS_PER_FIELD_V1 {
            return Err(MechanismCheckpointRenderError::limit(format!(
                "mechanism checkpoint field `{}` has an invalid bounded bin count",
                field.name
            )));
        }
        total_bins = total_bins.checked_add(field.bins.len()).ok_or_else(|| {
            MechanismCheckpointRenderError::limit(
                "mechanism checkpoint total bin count exceeds usize::MAX",
            )
        })?;
        if total_bins > MECHANISM_CHECKPOINT_MAX_TOTAL_BINS_V1 {
            return Err(MechanismCheckpointRenderError::limit(format!(
                "mechanism checkpoint total bin count exceeds {}",
                MECHANISM_CHECKPOINT_MAX_TOTAL_BINS_V1
            )));
        }
        validate_bin_field(summary, field, confirmed_signatures)?;
    }
    Ok(())
}

fn validate_bin_field(
    summary: &MechanismCheckpointSummaryV1,
    field: &MechanismCheckpointBinFieldV1,
    confirmed_signatures: u128,
) -> Result<(), MechanismCheckpointRenderError> {
    let unavailable = checked_add(
        field.replay_unavailable_cases,
        field.observation_unsupported_cases,
        "mechanism checkpoint unavailable field cases",
    )?;
    if unavailable != field.unavailable_cases {
        return Err(MechanismCheckpointRenderError::invalid(format!(
            "mechanism checkpoint field `{}` does not conserve unavailable reasons",
            field.name
        )));
    }
    let classified = checked_add(
        checked_add(
            field.binned_cases,
            field.outside_declared_bins_cases,
            "mechanism checkpoint classified field cases",
        )?,
        unavailable,
        "mechanism checkpoint classified field cases",
    )?;
    if classified != summary.traced_cases {
        return Err(MechanismCheckpointRenderError::invalid(format!(
            "mechanism checkpoint field `{}` does not conserve traced cases",
            field.name
        )));
    }

    let mut previous_upper = None;
    let mut binned_cases = 0_u128;
    for bin in field.bins.iter() {
        if bin.bin.lower_inclusive >= bin.bin.upper_exclusive
            || previous_upper.is_some_and(|upper| upper > bin.bin.lower_inclusive)
        {
            return Err(MechanismCheckpointRenderError::invalid(format!(
                "mechanism checkpoint field `{}` has empty, reversed or overlapping bins",
                field.name
            )));
        }
        previous_upper = Some(bin.bin.upper_exclusive);
        binned_cases = checked_add(
            binned_cases,
            bin.confirmed_case_support,
            "mechanism checkpoint declared-bin case support",
        )?;
        let confirmed = bin.mechanism_count.confirmed_lower_bound();
        if confirmed > bin.confirmed_case_support || confirmed > confirmed_signatures {
            return Err(MechanismCheckpointRenderError::invalid(format!(
                "mechanism checkpoint field `{}` confirms more bin mechanisms than supporting cases or signatures",
                field.name
            )));
        }
        let expected = expected_checkpoint_count(summary.status, unavailable, confirmed);
        if bin.mechanism_count != expected {
            return Err(MechanismCheckpointRenderError::invalid(format!(
                "mechanism checkpoint field `{}` uses bin certainty unsupported by closure and value availability",
                field.name
            )));
        }
    }
    if binned_cases != field.binned_cases {
        return Err(MechanismCheckpointRenderError::invalid(format!(
            "mechanism checkpoint field `{}` bins do not conserve binned cases",
            field.name
        )));
    }
    Ok(())
}

fn expected_checkpoint_count(
    status: MechanismEvidenceStatus,
    unavailable_cases: u128,
    confirmed: u128,
) -> MechanismCheckpointCountV1 {
    if status == MechanismEvidenceStatus::MatchingClosed && unavailable_cases == 0 {
        MechanismCheckpointCountV1::Exact(confirmed)
    } else if confirmed == 0 {
        MechanismCheckpointCountV1::Unknown {
            confirmed_lower_bound: 0,
        }
    } else {
        MechanismCheckpointCountV1::LowerBound(confirmed)
    }
}

fn mechanism_count(count: MechanismCount) -> MechanismCheckpointCountV1 {
    match count {
        MechanismCount::Exact(value) => MechanismCheckpointCountV1::Exact(value),
        MechanismCount::LowerBound(value) => MechanismCheckpointCountV1::LowerBound(value),
    }
}

fn write_bin_field(
    writer: &mut CanonicalJsonWriter,
    field: &MechanismCheckpointBinFieldV1,
) -> Result<(), MechanismCheckpointRenderError> {
    writer.raw(b"{")?;
    writer.member_string("name", &field.name)?;
    writer.raw(b",")?;
    writer.member_string("unit", "distinct_mechanism_signatures")?;
    writer.raw(b",")?;
    writer.member_bool("counts_are_non_additive", true)?;
    writer.raw(b",\"coverage\":{")?;
    writer.member_decimal("binned_cases", field.binned_cases)?;
    writer.raw(b",")?;
    writer.member_decimal(
        "outside_declared_bins_cases",
        field.outside_declared_bins_cases,
    )?;
    writer.raw(b",")?;
    writer.member_decimal("unavailable_cases", field.unavailable_cases)?;
    writer.raw(b",")?;
    writer.member_decimal("replay_unavailable_cases", field.replay_unavailable_cases)?;
    writer.raw(b",")?;
    writer.member_decimal(
        "observation_unsupported_cases",
        field.observation_unsupported_cases,
    )?;
    writer.raw(b"},\"bins\":[")?;
    for (index, bin) in field.bins.iter().enumerate() {
        if index != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{")?;
        writer.member_decimal("lower_inclusive", bin.bin.lower_inclusive)?;
        writer.raw(b",")?;
        writer.member_decimal("upper_exclusive", bin.bin.upper_exclusive)?;
        writer.raw(b",")?;
        writer.member_decimal("confirmed_case_support", bin.confirmed_case_support)?;
        writer.raw(b",\"mechanism_count\":")?;
        write_mechanism_count(writer, bin.mechanism_count)?;
        writer.raw(b"}")?;
    }
    writer.raw(b"]}")
}

fn write_mechanism_count(
    writer: &mut CanonicalJsonWriter,
    count: MechanismCheckpointCountV1,
) -> Result<(), MechanismCheckpointRenderError> {
    writer.raw(b"{")?;
    match count {
        MechanismCheckpointCountV1::Exact(value) => {
            writer.member_string("certainty", "exact")?;
            writer.raw(b",")?;
            writer.member_decimal("value", value)?;
        }
        MechanismCheckpointCountV1::LowerBound(value) => {
            writer.member_string("certainty", "lower_bound")?;
            writer.raw(b",")?;
            writer.member_decimal("value", value)?;
        }
        MechanismCheckpointCountV1::Unknown {
            confirmed_lower_bound,
        } => {
            writer.member_string("certainty", "unknown")?;
            writer.raw(b",\"value\":null,")?;
            writer.member_decimal("confirmed_lower_bound", confirmed_lower_bound)?;
        }
    }
    writer.raw(b"}")
}

fn evidence_status_name(status: MechanismEvidenceStatus) -> &'static str {
    match status {
        MechanismEvidenceStatus::ScopeOpen => "scope_open",
        MechanismEvidenceStatus::IncidenceOpen => "incidence_open",
        MechanismEvidenceStatus::MatchingClosed => "matching_closed",
    }
}

fn checked_add(
    left: u128,
    right: u128,
    what: &str,
) -> Result<u128, MechanismCheckpointRenderError> {
    left.checked_add(right)
        .ok_or_else(|| MechanismCheckpointRenderError::invalid(format!("{what} exceeds u128::MAX")))
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

struct CanonicalJsonWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl CanonicalJsonWriter {
    fn with_max_bytes(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), MechanismCheckpointRenderError> {
        let next_len = self.bytes.len().checked_add(value.len()).ok_or_else(|| {
            MechanismCheckpointRenderError::limit("canonical checkpoint JSON size overflow")
        })?;
        if next_len > self.max_bytes {
            return Err(MechanismCheckpointRenderError::limit(format!(
                "canonical checkpoint JSON exceeds {} bytes",
                self.max_bytes
            )));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), MechanismCheckpointRenderError> {
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

    fn decimal(&mut self, value: impl fmt::Display) -> Result<(), MechanismCheckpointRenderError> {
        self.string(&value.to_string())
    }

    fn member_string(
        &mut self,
        name: &str,
        value: &str,
    ) -> Result<(), MechanismCheckpointRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.string(value)
    }

    fn member_decimal(
        &mut self,
        name: &str,
        value: impl fmt::Display,
    ) -> Result<(), MechanismCheckpointRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.decimal(value)
    }

    fn member_optional_decimal<T: fmt::Display>(
        &mut self,
        name: &str,
        value: Option<T>,
    ) -> Result<(), MechanismCheckpointRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        if let Some(value) = value {
            self.decimal(value)
        } else {
            self.raw(b"null")
        }
    }

    fn member_bool(
        &mut self,
        name: &str,
        value: bool,
    ) -> Result<(), MechanismCheckpointRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.raw(if value { b"true" } else { b"false" })
    }

    fn member_u64(&mut self, name: &str, value: u64) -> Result<(), MechanismCheckpointRenderError> {
        self.string(name)?;
        self.raw(b":")?;
        self.raw(value.to_string().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::super::mechanism::MechanismNumericBin;
    use super::super::mechanism_stream::{MechanismCheckpointBinV1, MechanismCheckpointUntracedV1};
    use super::*;

    fn metadata() -> MechanismObservableCheckpointMetadataV1 {
        MechanismObservableCheckpointMetadataV1 {
            schema_digest: CanonicalDigest::from_sha256_bytes([1; 32]),
            run_id: ExploreRunId::from_lowercase_sha256(&"02".repeat(32)).unwrap(),
            sequence: 7,
            journal_head: JournalHead::from_lowercase_sha256(&"03".repeat(32)).unwrap(),
            evidence_root: EvidenceRoot::from_lowercase_sha256(&"04".repeat(32)).unwrap(),
            last_coverage_epoch: NonZeroU64::new(3),
            probe_milestone_complete: true,
            universe_case_count: 2,
            closed_case_count: 2,
        }
    }

    fn summary() -> MechanismCheckpointSummaryV1 {
        MechanismCheckpointSummaryV1 {
            checked_request_hash: [5; 32],
            observation_spec_hash: [6; 32],
            status: MechanismEvidenceStatus::MatchingClosed,
            target_cases: MechanismCount::Exact(1),
            traced_cases: 1,
            known_target_untraced: MechanismCheckpointUntracedV1 {
                total: 0,
                pending: 0,
                replay_unavailable: 0,
                observation_unsupported: 0,
            },
            mechanism_signatures: MechanismCheckpointCountV1::Exact(1),
            bin_fields: vec![MechanismCheckpointBinFieldV1 {
                name: "loss".into(),
                binned_cases: 1,
                outside_declared_bins_cases: 0,
                unavailable_cases: 0,
                replay_unavailable_cases: 0,
                observation_unsupported_cases: 0,
                bins: vec![MechanismCheckpointBinV1 {
                    bin: MechanismNumericBin::new(0, 50).unwrap(),
                    confirmed_case_support: 1,
                    mechanism_count: MechanismCheckpointCountV1::Exact(1),
                }]
                .into_boxed_slice(),
            }]
            .into_boxed_slice(),
        }
    }

    #[test]
    fn checkpoint_renderer_is_canonical_cursor_bound_json_line() {
        let rendered =
            render_mechanism_observable_checkpoint_json_line_v1(&metadata(), &summary()).unwrap();
        let expected = format!(
            "{{\"schema\":\"futuruna.explore.mechanism-checkpoint.v1\",\"schema_version\":1,\"schema_digest\":\"{}\",\"run\":{{\"run_id\":\"{}\",\"sequence\":\"7\",\"journal_head\":\"{}\",\"evidence_root\":\"{}\",\"lifecycle\":\"running\",\"probe_milestone_complete\":true,\"last_coverage_epoch\":\"3\"}},\"progress\":{{\"universe_case_count\":\"2\",\"closed_case_count\":\"2\",\"open_case_count\":\"0\"}},\"mechanism\":{{\"target\":\"matching_configurations\",\"checked_request_hash\":\"{}\",\"observation_spec_hash\":\"{}\",\"status\":\"matching_closed\",\"target_cases\":{{\"certainty\":\"exact\",\"value\":\"1\"}},\"traced_cases\":\"1\",\"known_target_untraced\":{{\"total\":\"0\",\"pending\":\"0\",\"replay_unavailable\":\"0\",\"observation_unsupported\":\"0\"}},\"mechanism_signatures\":{{\"certainty\":\"exact\",\"value\":\"1\"}},\"bin_fields\":[{{\"name\":\"loss\",\"unit\":\"distinct_mechanism_signatures\",\"counts_are_non_additive\":true,\"coverage\":{{\"binned_cases\":\"1\",\"outside_declared_bins_cases\":\"0\",\"unavailable_cases\":\"0\",\"replay_unavailable_cases\":\"0\",\"observation_unsupported_cases\":\"0\"}},\"bins\":[{{\"lower_inclusive\":\"0\",\"upper_exclusive\":\"50\",\"confirmed_case_support\":\"1\",\"mechanism_count\":{{\"certainty\":\"exact\",\"value\":\"1\"}}}}]}}]}}}}\n",
            "01".repeat(32),
            "02".repeat(32),
            "03".repeat(32),
            "04".repeat(32),
            "05".repeat(32),
            "06".repeat(32),
        );
        assert_eq!(String::from_utf8(rendered).unwrap(), expected);
    }

    #[test]
    fn checkpoint_renderer_rejects_nonconserving_summary_before_output() {
        let mut summary = summary();
        summary.bin_fields[0].binned_cases = 0;
        let error = render_mechanism_observable_checkpoint_json_line_v1(&metadata(), &summary)
            .expect_err("nonconserving checkpoint must fail closed");
        assert!(!error.is_capacity_limit());
        assert!(error.to_string().contains("conserve"));
    }

    #[test]
    fn checkpoint_renderer_requires_case_closure_exactly_with_target_closure() {
        let mut open_metadata = metadata();
        open_metadata.closed_case_count = 1;
        let error = render_mechanism_observable_checkpoint_json_line_v1(&open_metadata, &summary())
            .expect_err("an open case frontier cannot publish exact mechanism scope");
        assert!(error.to_string().contains("if and only if"));

        let mut open_summary = summary();
        open_summary.status = MechanismEvidenceStatus::ScopeOpen;
        open_summary.target_cases = MechanismCount::LowerBound(1);
        open_summary.mechanism_signatures = MechanismCheckpointCountV1::LowerBound(1);
        let error = render_mechanism_observable_checkpoint_json_line_v1(&metadata(), &open_summary)
            .expect_err("closed case classification cannot retain open mechanism scope");
        assert!(error.to_string().contains("if and only if"));
    }

    #[test]
    fn checkpoint_renderer_preserves_honest_zero_evidence_states() {
        let mut metadata = metadata();
        metadata.closed_case_count = 0;
        let mut zero = summary();
        zero.status = MechanismEvidenceStatus::ScopeOpen;
        zero.target_cases = MechanismCount::LowerBound(0);
        zero.traced_cases = 0;
        zero.mechanism_signatures = MechanismCheckpointCountV1::Unknown {
            confirmed_lower_bound: 0,
        };
        zero.bin_fields = Box::default();
        let scope_open = render_mechanism_observable_checkpoint_json_line_v1(&metadata, &zero)
            .expect("probe-complete scope-open zero evidence is observable");
        assert!(String::from_utf8(scope_open)
            .unwrap()
            .contains("\"certainty\":\"unknown\",\"value\":null"));

        metadata.closed_case_count = metadata.universe_case_count;
        zero.status = MechanismEvidenceStatus::MatchingClosed;
        zero.target_cases = MechanismCount::Exact(0);
        zero.mechanism_signatures = MechanismCheckpointCountV1::Exact(0);
        let closed = render_mechanism_observable_checkpoint_json_line_v1(&metadata, &zero)
            .expect("a fully classified empty target is exact zero");
        assert!(String::from_utf8(closed)
            .unwrap()
            .contains("\"mechanism_signatures\":{\"certainty\":\"exact\",\"value\":\"0\"}"));
    }

    #[test]
    fn checkpoint_writer_reports_capacity_without_returning_a_prefix() {
        let mut writer = CanonicalJsonWriter::with_max_bytes(1);
        let error = writer.raw(b"{}").expect_err("two bytes exceed limit");
        assert!(error.is_capacity_limit());
    }
}
