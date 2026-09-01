//! Fail-closed process boundary for a query-bound native classifier.
//!
//! V2 classifies one ordered batch from the checked source enumerator's finite
//! integer-factor values. Derived singleton bindings, composite Context/Before
//! values and the singleton successor are reconstructed inside the query-bound
//! executable. These inputs are operational only: the host retains every
//! semantic value and remains the sole producer of IDs and journal evidence.
//! The executable is valid only for the exact checked
//! program/relation/admission/question header carried in both directions. The
//! host never accepts a prefix: every response must decode completely before
//! any outcome is returned.
//!
//! Canonical request framing (all integers big-endian):
//! `request-magic | version | 4 * digest | factors:u32 | count:u32 |
//! count * factors * value:i64`.
//! The response mirrors that header and ends with exactly `count` outcome tags.
//! Digest order is program, relation, admission, then question.
//! Outcome tags are `1 = rejected`, `2 = admitted/not-selected`, and
//! `3 = admitted/selected`.

use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use std::{cmp, thread};

use crate::{CheckedExploreQueryView, Ty};

use super::relational_bounded_chunk_partition::RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1;
use super::relational_classified_sweep::{
    RelationalCheckedClassificationContext, RelationalClassifiedCaseOutcome,
    RelationalClassifiedSweepError, RelationalOrderedClassificationBackend,
    RelationalOrderedClassificationSubject,
};
use super::relational_executor::RelationalExpressionRuntime;
use super::{
    ExploreFiniteDomainIr, ExploreSourceBindingKindIr, ExploreSuccessorKindIr, ExploreValue,
};

/// Frozen wire constants shared with a generated V2 sidecar executable.
///
/// Identity digests are ordered program, relation, admission, question.
/// Counts and signed inputs are big-endian. No field is length-prefixed except
/// the subject count; request EOF and exact response length close each frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationalNativeClassifierProtocolV2;

impl RelationalNativeClassifierProtocolV2 {
    pub const VERSION: u32 = 2;
    pub const REQUEST_MAGIC: &'static [u8] = b"futuruna.explore.native-classifier.request.v2\0";
    pub const RESPONSE_MAGIC: &'static [u8] = b"futuruna.explore.native-classifier.response.v2\0";
    pub const IDENTITY_DIGEST_BYTES: usize = 32;
    pub const IDENTITY_DIGEST_COUNT: usize = 4;
    pub const COUNT_BYTES: usize = 4;
    pub const FACTOR_COUNT_BYTES: usize = 4;
    pub const FACTOR_INT_BYTES: usize = 8;
    pub const OUTCOME_BYTES: usize = 1;
    pub const MAX_FACTORS_PER_SUBJECT: usize = 32;
    pub const MAX_BATCH_SUBJECTS: usize = RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1 as usize;
    pub const OUTCOME_REJECTED: u8 = 0x01;
    pub const OUTCOME_ADMITTED_NOT_SELECTED: u8 = 0x02;
    pub const OUTCOME_ADMITTED_SELECTED: u8 = 0x03;
}

const REQUEST_MAGIC_V2: &[u8] = RelationalNativeClassifierProtocolV2::REQUEST_MAGIC;
const RESPONSE_MAGIC_V2: &[u8] = RelationalNativeClassifierProtocolV2::RESPONSE_MAGIC;
const DIGEST_BYTES: usize = RelationalNativeClassifierProtocolV2::IDENTITY_DIGEST_BYTES;
const U32_BYTES: usize = RelationalNativeClassifierProtocolV2::COUNT_BYTES;
const I64_BYTES: usize = RelationalNativeClassifierProtocolV2::FACTOR_INT_BYTES;
const RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2: usize =
    RelationalNativeClassifierProtocolV2::MAX_BATCH_SUBJECTS;
const INVOCATION_TIMEOUT_V2: Duration = Duration::from_secs(30);
const MAX_REQUEST_BYTES_V2: usize = REQUEST_MAGIC_V2.len()
    + U32_BYTES
    + RelationalNativeClassifierProtocolV2::IDENTITY_DIGEST_COUNT * DIGEST_BYTES
    + RelationalNativeClassifierProtocolV2::FACTOR_COUNT_BYTES
    + U32_BYTES
    + RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2
        * RelationalNativeClassifierProtocolV2::MAX_FACTORS_PER_SUBJECT
        * I64_BYTES;
const MAX_RESPONSE_BYTES_V2: usize = RESPONSE_MAGIC_V2.len()
    + U32_BYTES
    + RelationalNativeClassifierProtocolV2::IDENTITY_DIGEST_COUNT * DIGEST_BYTES
    + U32_BYTES
    + RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2
        * RelationalNativeClassifierProtocolV2::OUTCOME_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalNativeClassifierIdentityV2 {
    program_hash: [u8; DIGEST_BYTES],
    relation_id: [u8; DIGEST_BYTES],
    admission_id: [u8; DIGEST_BYTES],
    question_id: [u8; DIGEST_BYTES],
}

impl RelationalNativeClassifierIdentityV2 {
    fn from_checked(
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalNativeClassifierUnavailable> {
        Ok(Self {
            program_hash: decode_lowercase_sha256(checked.program_hash())
                .ok_or(RelationalNativeClassifierUnavailable::InvalidCheckedProgramHash)?,
            relation_id: checked.relation_id().bytes(),
            admission_id: checked.admission_id().bytes(),
            question_id: checked.question_id().bytes(),
        })
    }

    fn encode_into(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.program_hash);
        output.extend_from_slice(&self.relation_id);
        output.extend_from_slice(&self.admission_id);
        output.extend_from_slice(&self.question_id);
    }
}

fn finite_input_binding_indices_from_checked(
    checked: &CheckedExploreQueryView<'_>,
) -> Result<Box<[usize]>, RelationalNativeClassifierUnavailable> {
    checked
        .closed_query
        .validate()
        .map_err(|_| RelationalNativeClassifierUnavailable::InvalidFiniteInputShape)?;
    if !matches!(
        &checked.closed_query.successor.kind,
        ExploreSuccessorKindIr::Singleton { .. }
    ) {
        return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
    }
    let mut indices = Vec::new();
    for (position, binding) in checked.closed_query.source.bindings.iter().enumerate() {
        if binding.binding_index != position {
            return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
        }
        if let ExploreSourceBindingKindIr::Finite { domain } = &binding.kind {
            if !binding.dependencies.is_empty()
                || !matches!(domain, ExploreFiniteDomainIr::IntRange { .. })
                || !matches!(&binding.value_ty, Ty::Name(name) if matches!(name.as_str(), "Int" | "Heltal"))
            {
                return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
            }
            indices.push(position);
        }
    }
    if indices.is_empty()
        || indices.len() > RelationalNativeClassifierProtocolV2::MAX_FACTORS_PER_SUBJECT
    {
        return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
    }
    Ok(indices.into_boxed_slice())
}

/// One query-bound executable speaking the strict native-classifier V2 frame.
#[derive(Clone, Debug)]
pub(crate) struct RelationalNativeClassifierV2 {
    executable: PathBuf,
    identity: RelationalNativeClassifierIdentityV2,
    finite_input_binding_indices: Arc<[usize]>,
    enabled: Arc<AtomicBool>,
    parity_checked: Arc<AtomicBool>,
}

impl RelationalNativeClassifierV2 {
    pub(crate) fn for_checked_query(
        executable: impl Into<PathBuf>,
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalNativeClassifierUnavailable> {
        let finite_input_binding_indices = finite_input_binding_indices_from_checked(checked)?;
        Ok(Self {
            executable: executable.into(),
            identity: RelationalNativeClassifierIdentityV2::from_checked(checked)?,
            finite_input_binding_indices: finite_input_binding_indices.into(),
            enabled: Arc::new(AtomicBool::new(true)),
            parity_checked: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }

    /// Whether this process-local accelerator may still be attempted.
    ///
    /// Unavailability is sticky across clones so callers can route later
    /// batches to another exact backend without paying one checked native
    /// fallback per batch. This flag is operational only and carries no
    /// semantic or evidence authority.
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    pub(crate) fn classify_ordered_batch(
        &self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
    ) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalNativeClassifierUnavailable> {
        if !self.is_enabled() {
            return Err(RelationalNativeClassifierUnavailable::DisabledAfterUnavailable);
        }
        let request = encode_request(self.identity, &self.finite_input_binding_indices, subjects)?;
        let response = invoke_once(&self.executable, &request)?;
        decode_response(self.identity, subjects.len(), &response)
    }

    /// Execute native classification and atomically fall back to a checked
    /// whole batch. The first successful native batch is compared with that
    /// checked batch before any native outcome can be accepted.
    pub(crate) fn classify_or_fallback<E>(
        &self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
        fallback: impl FnOnce() -> Result<Box<[RelationalClassifiedCaseOutcome]>, E>,
    ) -> Result<
        (
            Box<[RelationalClassifiedCaseOutcome]>,
            Option<RelationalNativeClassifierUnavailable>,
        ),
        E,
    > {
        match self.classify_ordered_batch(subjects) {
            Ok(outcomes) if self.parity_checked() => Ok((outcomes, None)),
            Ok(outcomes) => {
                let checked_outcomes = fallback()?;
                if checked_outcomes != outcomes {
                    let unavailable = RelationalNativeClassifierUnavailable::ParityCanaryMismatch;
                    if self.disable() {
                        trace_native_classifier_unavailable(&unavailable);
                    }
                    return Ok((checked_outcomes, Some(unavailable)));
                }
                self.mark_parity_checked();
                Ok((outcomes, None))
            }
            Err(unavailable) => {
                if self.disable() {
                    trace_native_classifier_unavailable(&unavailable);
                }
                Ok((fallback()?, Some(unavailable)))
            }
        }
    }

    fn disable(&self) -> bool {
        self.enabled.swap(false, Ordering::AcqRel)
    }

    fn parity_checked(&self) -> bool {
        self.parity_checked.load(Ordering::Acquire)
    }

    fn mark_parity_checked(&self) {
        self.parity_checked.store(true, Ordering::Release);
    }
}

/// Native first, with one atomic interpreter fallback for any unavailable V2
/// execution. A malformed response can never contribute a prefix of outcomes.
#[derive(Clone, Debug)]
pub(crate) struct RelationalNativeClassifierFallbackBackendV2 {
    native: RelationalNativeClassifierV2,
    last_unavailable: Option<RelationalNativeClassifierUnavailable>,
}

impl RelationalNativeClassifierFallbackBackendV2 {
    pub(crate) fn new(native: RelationalNativeClassifierV2) -> Self {
        Self {
            native,
            last_unavailable: None,
        }
    }

    pub(crate) fn last_unavailable(&self) -> Option<&RelationalNativeClassifierUnavailable> {
        self.last_unavailable.as_ref()
    }
}

impl RelationalOrderedClassificationBackend for RelationalNativeClassifierFallbackBackendV2 {
    fn classify_ordered_batch<R: RelationalExpressionRuntime>(
        &mut self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
        checked: &mut RelationalCheckedClassificationContext<'_, '_, '_, R>,
    ) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalClassifiedSweepError> {
        let (outcomes, unavailable) = self.native.classify_or_fallback(subjects, || {
            subjects
                .iter()
                .copied()
                .map(|subject| checked.classify(subject))
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice)
        })?;
        self.last_unavailable = unavailable;
        Ok(outcomes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalNativeClassifierIdentityFieldV2 {
    ProgramHash,
    Relation,
    Admission,
    Question,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalNativeClassifierUnavailable {
    DisabledAfterUnavailable,
    ParityCanaryMismatch,
    InvalidCheckedProgramHash,
    InvalidFiniteInputShape,
    BatchTooLarge {
        actual: usize,
        maximum: usize,
    },
    UnsupportedFiniteInputValue {
        subject_index: usize,
        binding_index: usize,
    },
    RequestTooLarge,
    SpawnFailed,
    RequestPipeUnavailable,
    RequestWriteFailed,
    ResponsePipeUnavailable,
    ResponseReadFailed,
    ResponseTooLarge,
    InvocationTimedOut,
    WaitFailed,
    UnsuccessfulExit {
        code: Option<i32>,
    },
    TruncatedResponse {
        field: &'static str,
    },
    InvalidResponseMagic,
    UnsupportedResponseVersion(u32),
    IdentityMismatch(RelationalNativeClassifierIdentityFieldV2),
    OutcomeCountMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidOutcomeTag {
        index: usize,
        tag: u8,
    },
    TrailingResponseBytes {
        count: usize,
    },
}

impl fmt::Display for RelationalNativeClassifierUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native classifier unavailable: {self:?}")
    }
}

impl Error for RelationalNativeClassifierUnavailable {}

fn trace_native_classifier_unavailable(unavailable: &RelationalNativeClassifierUnavailable) {
    if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
        eprintln!("Explore native classifier disabled; using checked interpreter: {unavailable}");
    }
}

fn encode_request(
    identity: RelationalNativeClassifierIdentityV2,
    finite_input_binding_indices: &[usize],
    subjects: &[RelationalOrderedClassificationSubject<'_>],
) -> Result<Vec<u8>, RelationalNativeClassifierUnavailable> {
    if subjects.len() > RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2 {
        return Err(RelationalNativeClassifierUnavailable::BatchTooLarge {
            actual: subjects.len(),
            maximum: RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2,
        });
    }
    if finite_input_binding_indices.is_empty()
        || finite_input_binding_indices.len()
            > RelationalNativeClassifierProtocolV2::MAX_FACTORS_PER_SUBJECT
        || finite_input_binding_indices
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(RelationalNativeClassifierUnavailable::InvalidFiniteInputShape);
    }
    let count = u32::try_from(subjects.len()).map_err(|_| {
        RelationalNativeClassifierUnavailable::BatchTooLarge {
            actual: subjects.len(),
            maximum: RELATIONAL_NATIVE_CLASSIFIER_MAX_BATCH_SUBJECTS_V2,
        }
    })?;
    let factor_count = u32::try_from(finite_input_binding_indices.len())
        .map_err(|_| RelationalNativeClassifierUnavailable::InvalidFiniteInputShape)?;
    let mut request = Vec::with_capacity(MAX_REQUEST_BYTES_V2);
    request.extend_from_slice(REQUEST_MAGIC_V2);
    request.extend_from_slice(&RelationalNativeClassifierProtocolV2::VERSION.to_be_bytes());
    identity.encode_into(&mut request);
    request.extend_from_slice(&factor_count.to_be_bytes());
    request.extend_from_slice(&count.to_be_bytes());
    for (subject_index, subject) in subjects.iter().copied().enumerate() {
        for &binding_index in finite_input_binding_indices {
            let Some(ExploreValue::Int(value)) = subject.source_binding(binding_index) else {
                return Err(
                    RelationalNativeClassifierUnavailable::UnsupportedFiniteInputValue {
                        subject_index,
                        binding_index,
                    },
                );
            };
            request.extend_from_slice(&value.to_be_bytes());
        }
    }
    if request.len() > MAX_REQUEST_BYTES_V2 {
        return Err(RelationalNativeClassifierUnavailable::RequestTooLarge);
    }
    Ok(request)
}

fn invoke_once(
    executable: &Path,
    request: &[u8],
) -> Result<Vec<u8>, RelationalNativeClassifierUnavailable> {
    let deadline = Instant::now() + INVOCATION_TIMEOUT_V2;
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| RelationalNativeClassifierUnavailable::SpawnFailed)?;

    let Some(mut stdin) = child.stdin.take() else {
        terminate(&mut child);
        return Err(RelationalNativeClassifierUnavailable::RequestPipeUnavailable);
    };
    if stdin.write_all(request).is_err() {
        drop(stdin);
        terminate(&mut child);
        return Err(RelationalNativeClassifierUnavailable::RequestWriteFailed);
    }
    drop(stdin);

    let Some(stdout) = child.stdout.take() else {
        terminate(&mut child);
        return Err(RelationalNativeClassifierUnavailable::ResponsePipeUnavailable);
    };
    let (response_sender, response_receiver) = mpsc::sync_channel(1);
    // This handle is deliberately never joined synchronously. A successful
    // receive means the bounded read has completed. On timeout or process
    // failure a descendant may still hold the inherited pipe open, so dropping
    // the handle detaches that one blocked reader instead of hanging fallback.
    // The caller disables the classifier's shared latch before another
    // sequential batch can invoke it, preventing detached-reader accumulation.
    let _reader = match thread::Builder::new()
        .name("futuruna-native-classifier".to_owned())
        .spawn(move || {
            let response = read_bounded_response(stdout, MAX_RESPONSE_BYTES_V2);
            let _ = response_sender.send(response);
        }) {
        Ok(reader) => reader,
        Err(_) => {
            terminate(&mut child);
            return Err(RelationalNativeClassifierUnavailable::ResponseReadFailed);
        }
    };

    let mut response = None;
    let status = loop {
        if response.is_none() {
            match response_receiver.try_recv() {
                Ok(Ok(received)) => {
                    if received.overflowed {
                        terminate(&mut child);
                        return Err(RelationalNativeClassifierUnavailable::ResponseTooLarge);
                    }
                    response = Some(received.bytes);
                }
                Ok(Err(unavailable)) => {
                    terminate(&mut child);
                    return Err(unavailable);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    terminate(&mut child);
                    return Err(RelationalNativeClassifierUnavailable::ResponseReadFailed);
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    terminate(&mut child);
                    return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
                }
                thread::sleep(cmp::min(
                    Duration::from_millis(5),
                    deadline.saturating_duration_since(now),
                ));
            }
            Err(_) => {
                terminate(&mut child);
                return Err(RelationalNativeClassifierUnavailable::WaitFailed);
            }
        }
    };
    if !status.success() {
        return Err(RelationalNativeClassifierUnavailable::UnsuccessfulExit {
            code: status.code(),
        });
    }
    let response = match response {
        Some(response) => response,
        None => {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
            };
            match response_receiver.recv_timeout(remaining) {
                Ok(Ok(received)) => {
                    if received.overflowed {
                        return Err(RelationalNativeClassifierUnavailable::ResponseTooLarge);
                    }
                    received.bytes
                }
                Ok(Err(unavailable)) => return Err(unavailable),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(RelationalNativeClassifierUnavailable::ResponseReadFailed);
                }
            }
        }
    };
    if Instant::now() > deadline {
        return Err(RelationalNativeClassifierUnavailable::InvocationTimedOut);
    }
    Ok(response)
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct BoundedResponse {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn read_bounded_response(
    mut stdout: std::process::ChildStdout,
    limit: usize,
) -> Result<BoundedResponse, RelationalNativeClassifierUnavailable> {
    let mut bytes = Vec::with_capacity(limit);
    let mut overflowed = false;
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stdout
            .read(&mut buffer)
            .map_err(|_| RelationalNativeClassifierUnavailable::ResponseReadFailed)?;
        if count == 0 {
            break;
        }
        let retained = cmp::min(limit.saturating_sub(bytes.len()), count);
        bytes.extend_from_slice(&buffer[..retained]);
        overflowed |= retained != count;
        if overflowed {
            break;
        }
    }
    Ok(BoundedResponse { bytes, overflowed })
}

fn decode_response(
    expected_identity: RelationalNativeClassifierIdentityV2,
    expected_count: usize,
    response: &[u8],
) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalNativeClassifierUnavailable> {
    let mut decoder = ResponseDecoder::new(response);
    if decoder.take(RESPONSE_MAGIC_V2.len(), "magic")? != RESPONSE_MAGIC_V2 {
        return Err(RelationalNativeClassifierUnavailable::InvalidResponseMagic);
    }
    let version = decoder.u32("version")?;
    if version != RelationalNativeClassifierProtocolV2::VERSION {
        return Err(RelationalNativeClassifierUnavailable::UnsupportedResponseVersion(version));
    }
    let actual_identity = RelationalNativeClassifierIdentityV2 {
        program_hash: decoder.digest("program hash")?,
        relation_id: decoder.digest("relation id")?,
        admission_id: decoder.digest("admission id")?,
        question_id: decoder.digest("question id")?,
    };
    for (field, matches) in [
        (
            RelationalNativeClassifierIdentityFieldV2::ProgramHash,
            actual_identity.program_hash == expected_identity.program_hash,
        ),
        (
            RelationalNativeClassifierIdentityFieldV2::Relation,
            actual_identity.relation_id == expected_identity.relation_id,
        ),
        (
            RelationalNativeClassifierIdentityFieldV2::Admission,
            actual_identity.admission_id == expected_identity.admission_id,
        ),
        (
            RelationalNativeClassifierIdentityFieldV2::Question,
            actual_identity.question_id == expected_identity.question_id,
        ),
    ] {
        if !matches {
            return Err(RelationalNativeClassifierUnavailable::IdentityMismatch(
                field,
            ));
        }
    }
    let actual_count = usize::try_from(decoder.u32("outcome count")?).map_err(|_| {
        RelationalNativeClassifierUnavailable::OutcomeCountMismatch {
            expected: expected_count,
            actual: usize::MAX,
        }
    })?;
    if actual_count != expected_count {
        return Err(
            RelationalNativeClassifierUnavailable::OutcomeCountMismatch {
                expected: expected_count,
                actual: actual_count,
            },
        );
    }
    let mut outcomes = Vec::with_capacity(actual_count);
    for index in 0..actual_count {
        let tag = decoder.u8("outcome")?;
        let outcome = RelationalClassifiedCaseOutcome::from_codec_tag(tag)
            .ok_or(RelationalNativeClassifierUnavailable::InvalidOutcomeTag { index, tag })?;
        outcomes.push(outcome);
    }
    if decoder.remaining() != 0 {
        return Err(
            RelationalNativeClassifierUnavailable::TrailingResponseBytes {
                count: decoder.remaining(),
            },
        );
    }
    Ok(outcomes.into_boxed_slice())
}

struct ResponseDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ResponseDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(
        &mut self,
        count: usize,
        field: &'static str,
    ) -> Result<&'a [u8], RelationalNativeClassifierUnavailable> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RelationalNativeClassifierUnavailable::TruncatedResponse { field })?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RelationalNativeClassifierUnavailable::TruncatedResponse { field })?;
        self.cursor = end;
        Ok(value)
    }

    fn digest(
        &mut self,
        field: &'static str,
    ) -> Result<[u8; DIGEST_BYTES], RelationalNativeClassifierUnavailable> {
        let mut digest = [0_u8; DIGEST_BYTES];
        digest.copy_from_slice(self.take(DIGEST_BYTES, field)?);
        Ok(digest)
    }

    fn u32(&mut self, field: &'static str) -> Result<u32, RelationalNativeClassifierUnavailable> {
        let mut bytes = [0_u8; U32_BYTES];
        bytes.copy_from_slice(self.take(U32_BYTES, field)?);
        Ok(u32::from_be_bytes(bytes))
    }

    fn u8(&mut self, field: &'static str) -> Result<u8, RelationalNativeClassifierUnavailable> {
        Ok(self.take(1, field)?[0])
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.cursor
    }
}

fn decode_lowercase_sha256(value: &str) -> Option<[u8; DIGEST_BYTES]> {
    if value.len() != DIGEST_BYTES * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }
    let mut digest = [0_u8; DIGEST_BYTES];
    for (index, output) in digest.iter_mut().enumerate() {
        let high = decode_hex(value.as_bytes()[index * 2])?;
        let low = decode_hex(value.as_bytes()[index * 2 + 1])?;
        *output = (high << 4) | low;
    }
    Some(digest)
}

const fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
